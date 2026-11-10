//! Freeze data contracts, formal witness identities and callables in one declaration authority.
use super::{
    Callable, DeclarationId, FunctionConstraint, FunctionInput, Parameter, Program, Reservation,
    ValueDefinition, data::resolved_data, functions, records,
};
use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, ArenaBuilder, Symbol, SymbolTable, TypeKind,
    ValueFunctionOrigin, ValueInvocation,
};
use std::collections::BTreeMap;

pub(super) fn publish(
    symbols: SymbolTable,
    reserved: &Arena<AccessCallableId, Reservation>,
    namespace: BTreeMap<Symbol, DeclarationId>,
    records: records::Validated,
) -> Program {
    let (base_types, records) = records.publish();
    let base_types = functions::publish_witnesses(
        &base_types,
        reserved.iter().filter_map(|(id, r)| {
            matches!(
                r.definition.as_ref()?.kind,
                super::CallableKind::Function(_)
            )
            .then_some(id)
        }),
    );
    let mut types = base_types.transaction();
    let mut parameters = ArenaBuilder::<AccessParameterId, Parameter>::new();
    let callables = reserved.map(|id, reservation| {
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        let parameter_ids = definition
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let next = parameters.next_id();
                let (data, contract) = publish_value(
                    &mut types,
                    &records,
                    ValueFunctionOrigin::Parameter(next),
                    &parameter.data,
                );
                parameters.insert(Parameter {
                    owner: id,
                    name: parameter.name,
                    position,
                    data,
                    operation: parameter.operation,
                    contract,
                })
            })
            .collect::<Box<[_]>>();
        let (result, result_contract) = publish_value(
            &mut types,
            &records,
            ValueFunctionOrigin::Result(id),
            &definition.result_data,
        );
        Callable {
            name: reservation.name,
            kind: definition.kind,
            parameters: parameter_ids,
            result,
            result_contract,
        }
    });
    let types = types
        .commit(&base_types)
        .expect("exact declaration lineage");
    let header_type_count = types.store().type_count();
    Program {
        closures: BTreeMap::new(),
        symbols: symbols.into(),
        types,
        header_type_count,
        callables: callables.into(),
        parameters: parameters.finish().into(),
        namespace: namespace.into(),
        records: records.into(),
    }
}

fn publish_value(
    types: &mut nocter_model::TypeTransaction,
    records: &records::Schemas,
    origin: ValueFunctionOrigin,
    definition: &ValueDefinition,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let definition = match definition {
        ValueDefinition::Data(data) => {
            return (
                resolved_data(
                    types,
                    |id| records.records.get(id).expect("validated record").data(),
                    data,
                ),
                None,
            );
        }
        ValueDefinition::Function(definition) => definition,
    };
    let contract = FunctionConstraint {
        invocation: definition.invocation,
        inputs: definition
            .inputs
            .iter()
            .map(|(operation, data)| FunctionInput {
                data: resolved_data(
                    types,
                    |id| records.records.get(id).expect("validated record").data(),
                    data,
                ),
                operation: *operation,
            })
            .collect(),
        result: resolved_data(
            types,
            |id| records.records.get(id).expect("validated record").data(),
            &definition.result,
        ),
    };
    let mut selected = None;
    for invocation in [
        ValueInvocation::Read,
        ValueInvocation::Edit,
        ValueInvocation::Take,
    ] {
        if !contract.invocation.can_supply(invocation) {
            continue;
        }
        let data = types
            .intern(TypeKind::ValueFunction { origin, invocation })
            .expect("witness is a leaf");
        if invocation == contract.invocation {
            selected = Some(data);
        }
    }
    (selected.expect("declared invocation view"), Some(contract))
}
