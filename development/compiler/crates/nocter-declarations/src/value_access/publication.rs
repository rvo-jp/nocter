//! Freeze data contracts, formal witness identities and callables in one declaration authority.
use super::{
    Callable, FunctionConstraint, FunctionInput, Parameter, Program, Reservation, ValueDefinition,
    data::resolved_data, functions, records,
};
use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, ArenaBuilder, Symbol, SymbolTable, TypeKind,
    ValueFunctionOrigin, ValueInvocation,
};
use std::collections::BTreeMap;

pub(super) fn publish(
    symbols: SymbolTable,
    reserved: &Arena<AccessCallableId, Reservation>,
    namespace: BTreeMap<Symbol, AccessCallableId>,
    records: records::Validated,
) -> Program {
    let (base_types, records) = records.publish();
    let base_types = functions::publish_witnesses(&base_types, reserved.iter().map(|(id, _)| id));
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
                    &base_types,
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
            &base_types,
            &mut types,
            &records,
            ValueFunctionOrigin::Result(id),
            &definition.result_data,
        );
        Callable {
            name: reservation.name,
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
    base: &nocter_model::TypeAuthority,
    types: &mut nocter_model::TypeTransaction,
    records: &records::Schemas,
    origin: ValueFunctionOrigin,
    definition: &ValueDefinition,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let definition = match definition {
        ValueDefinition::Data(data) => return (resolved_data(base, records, *data), None),
        ValueDefinition::Function(definition) => definition,
    };
    let contract = FunctionConstraint {
        invocation: definition.invocation,
        inputs: definition
            .inputs
            .iter()
            .map(|&(operation, data)| FunctionInput {
                data: resolved_data(base, records, data),
                operation,
            })
            .collect(),
        result: resolved_data(base, records, definition.result),
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
