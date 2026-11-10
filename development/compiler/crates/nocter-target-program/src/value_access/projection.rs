//! Mechanical witness/data substitution, consuming checked binding edges rather than lookup inputs.
use super::{ConcreteWitness, Failure, InstanceKey};
use nocter_checking::value_access::{
    Argument,
    accepted::{DataEvidence, Unit},
    flow,
    flow::resources::payload,
};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{TypeId, ValueCode, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn witness(
    key: &InstanceKey,
    selected: FunctionWitness,
) -> Result<ConcreteWitness, Failure> {
    let code = match selected.origin {
        ValueFunctionOrigin::Closure(id) => ValueCode::Closure(id),
        ValueFunctionOrigin::Declaration(id) => ValueCode::Declaration(id),
        ValueFunctionOrigin::Result(_) | nocter_model::ValueFunctionOrigin::Application(_) => {
            return Err(Failure::InvalidAcceptedUnit);
        }
        ValueFunctionOrigin::Parameter(id) => {
            let supplied = key.witnesses.get(&id).ok_or(Failure::InvalidAcceptedUnit)?;
            if !supplied.invocation.can_supply(selected.invocation) {
                return Err(Failure::InvalidAcceptedUnit);
            }
            supplied.code
        }
    };
    Ok(ConcreteWitness {
        code,
        invocation: selected.invocation,
    })
}

pub(super) fn callee(
    program: &Program,
    caller: &InstanceKey,
    target: ValueFunctionOrigin,
    arguments: &[Argument],
) -> Result<InstanceKey, Failure> {
    let code = match target {
        ValueFunctionOrigin::Closure(id) => ValueCode::Closure(id),
        ValueFunctionOrigin::Declaration(id) => ValueCode::Declaration(id),
        ValueFunctionOrigin::Result(_) | nocter_model::ValueFunctionOrigin::Application(_) => {
            return Err(Failure::InvalidAcceptedUnit);
        }
        ValueFunctionOrigin::Parameter(id) => {
            caller
                .witnesses
                .get(&id)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .code
        }
    };
    let mut witnesses = BTreeMap::new();
    for argument in arguments {
        if let Some(binding) = &argument.binding {
            let resolved = witness(caller, binding.witness)?;
            if witnesses.insert(binding.parameter, resolved).is_some() {
                return Err(Failure::InvalidAcceptedUnit);
            }
        }
    }
    let key = InstanceKey { code, witnesses };
    validate_key(program, &key)?;
    Ok(key)
}

/// Validate identity-domain coverage, without rechecking signatures or choosing a call candidate.
pub(super) fn validate_key(program: &Program, key: &InstanceKey) -> Result<(), Failure> {
    let mut expected = BTreeSet::new();
    match key.code {
        ValueCode::Declaration(id) => {
            let declaration = program
                .codes()
                .get(id)
                .ok_or(Failure::InvalidAcceptedUnit)?;
            for &id in declaration.parameters() {
                let parameter = program
                    .parameters()
                    .get(id)
                    .ok_or(Failure::InvalidAcceptedUnit)?;
                if parameter.contract().is_some() {
                    expected.insert(id);
                }
            }
        }
        ValueCode::Closure(id) => {
            program.closure(id).ok_or(Failure::InvalidAcceptedUnit)?;
        }
    }
    if expected != key.witnesses.keys().copied().collect() {
        return Err(Failure::InvalidAcceptedUnit);
    }
    Ok(())
}

pub(super) fn data(
    unit: &Unit,
    key: &InstanceKey,
    body: &flow::Query,
) -> Result<BTreeMap<TypeId, DataEvidence>, Failure> {
    let program = unit.program();
    let typed = body.typed().body();
    let mut pending = BTreeSet::new();
    pending.extend(typed.locals().iter().map(|(_, local)| local.data));
    pending.extend(typed.places().iter().map(|(_, place)| place.data));
    pending.extend(
        typed
            .expressions()
            .iter()
            .filter_map(|(_, e)| e.ty.access().map(nocter_model::ValueAccess::data)),
    );
    pending.extend(
        typed
            .blocks()
            .iter()
            .filter_map(|(_, b)| b.ty.access().map(nocter_model::ValueAccess::data)),
    );
    pending.insert(body.typed().result_data());
    let mut concrete = BTreeMap::new();
    while let Some(source) = pending.pop_first() {
        if concrete.contains_key(&source) {
            continue;
        }
        let evidence = unit
            .data_evidence(source)
            .map_err(|_| Failure::InvalidAcceptedUnit)?;
        if matches!(evidence, DataEvidence::Unproduced(_)) {
            concrete.insert(source, evidence);
            continue;
        }
        let target = match program
            .data_schema(source)
            .ok_or(Failure::InvalidAcceptedUnit)?
        {
            PayloadSchema::Function(selected) => {
                let selected = witness(key, selected)?;
                if let ValueCode::Closure(id) = selected.code {
                    pending.insert(
                        program
                            .closure(id)
                            .ok_or(Failure::InvalidAcceptedUnit)?
                            .environment(),
                    );
                }
                program
                    .function_data(FunctionWitness {
                        origin: selected.code.origin(),
                        invocation: selected.invocation,
                    })
                    .ok_or(Failure::InvalidAcceptedUnit)?
            }
            PayloadSchema::Aggregate(aggregate) => {
                pending.extend(aggregate.components().map(|component| component.data));
                source
            }
            PayloadSchema::Builtin(_) => source,
        };
        let contract = payload(program, target).map_err(|_| Failure::InvalidAcceptedUnit)?;
        concrete.insert(source, DataEvidence::Payload(contract));
    }
    Ok(concrete)
}
