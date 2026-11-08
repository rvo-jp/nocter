//! Mechanical witness/data substitution, consuming checked binding edges rather than lookup inputs.
use super::{ConcreteWitness, Failure, InstanceKey};
use nocter_checking::value_access::{
    Argument, flow,
    flow::resources::{Payload, payload},
};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{TypeId, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn witness(
    key: &InstanceKey,
    selected: FunctionWitness,
) -> Result<ConcreteWitness, Failure> {
    let declaration = match selected.origin {
        ValueFunctionOrigin::Declaration(id) => id,
        ValueFunctionOrigin::Parameter(id) => {
            let supplied = key.witnesses.get(&id).ok_or(Failure::InvalidAcceptedUnit)?;
            if !supplied.invocation.can_supply(selected.invocation) {
                return Err(Failure::InvalidAcceptedUnit);
            }
            supplied.declaration
        }
    };
    Ok(ConcreteWitness {
        declaration,
        invocation: selected.invocation,
    })
}

pub(super) fn callee(
    program: &Program,
    caller: &InstanceKey,
    target: ValueFunctionOrigin,
    arguments: &[Argument],
) -> Result<InstanceKey, Failure> {
    let callable = match target {
        ValueFunctionOrigin::Declaration(id) => id,
        ValueFunctionOrigin::Parameter(id) => {
            caller
                .witnesses
                .get(&id)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .declaration
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
    let key = InstanceKey {
        callable,
        witnesses,
    };
    validate_key(program, &key)?;
    Ok(key)
}

/// Validate identity-domain coverage, without rechecking signatures or choosing a call candidate.
pub(super) fn validate_key(program: &Program, key: &InstanceKey) -> Result<(), Failure> {
    let declaration = program
        .callables()
        .get(key.callable)
        .ok_or(Failure::InvalidAcceptedUnit)?;
    let mut expected = BTreeSet::new();
    for &id in declaration.parameters() {
        let parameter = program
            .parameters()
            .get(id)
            .ok_or(Failure::InvalidAcceptedUnit)?;
        if parameter.contract().is_some() {
            expected.insert(id);
        }
    }
    if expected != key.witnesses.keys().copied().collect() {
        return Err(Failure::InvalidAcceptedUnit);
    }
    Ok(())
}

pub(super) fn data(
    program: &Program,
    key: &InstanceKey,
    body: &flow::Query,
) -> Result<BTreeMap<TypeId, Payload>, Failure> {
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
    pending.insert(
        program
            .callables()
            .get(key.callable)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .result(),
    );
    let mut concrete = BTreeMap::new();
    while let Some(source) = pending.pop_first() {
        if concrete.contains_key(&source) {
            continue;
        }
        let target = match program
            .data_schema(source)
            .ok_or(Failure::InvalidAcceptedUnit)?
        {
            PayloadSchema::Function(selected) => {
                let selected = witness(key, selected)?;
                program
                    .function_data(FunctionWitness {
                        origin: ValueFunctionOrigin::Declaration(selected.declaration),
                        invocation: selected.invocation,
                    })
                    .ok_or(Failure::InvalidAcceptedUnit)?
            }
            PayloadSchema::Record(record) => {
                for field in record.fields() {
                    pending.insert(
                        program
                            .fields()
                            .get(*field)
                            .ok_or(Failure::InvalidAcceptedUnit)?
                            .data(),
                    );
                }
                source
            }
            PayloadSchema::Builtin(_) => source,
        };
        let contract = payload(program, target).map_err(|_| Failure::InvalidAcceptedUnit)?;
        concrete.insert(source, contract);
    }
    Ok(concrete)
}
