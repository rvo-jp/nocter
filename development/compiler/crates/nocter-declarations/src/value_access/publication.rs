//! Freeze data contracts, formal witness identities and callables in one declaration authority.
use super::{
    Callable, FunctionConstraint, FunctionInput, Parameter, ParameterData, Program, Reservation,
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
                let (data, contract) = match &parameter.data {
                    ParameterData::Data(data) => {
                        (resolved_data(&base_types, &records, *data), None)
                    }
                    ParameterData::Function(contract) => {
                        let contract = FunctionConstraint {
                            invocation: contract.invocation,
                            inputs: contract
                                .inputs
                                .iter()
                                .map(|&(operation, data)| FunctionInput {
                                    data: resolved_data(&base_types, &records, data),
                                    operation,
                                })
                                .collect(),
                            result: resolved_data(&base_types, &records, contract.result),
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
                                .intern(TypeKind::ValueFunction {
                                    origin: ValueFunctionOrigin::Parameter(next),
                                    invocation,
                                })
                                .expect("formal witness is a leaf");
                            if invocation == contract.invocation {
                                selected = Some(data);
                            }
                        }
                        (selected.expect("declared invocation view"), Some(contract))
                    }
                };
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
        Callable {
            name: reservation.name,
            parameters: parameter_ids,
            result: resolved_data(&base_types, &records, definition.result_data),
        }
    });
    Program {
        symbols,
        types: types
            .commit(&base_types)
            .expect("exact declaration lineage"),
        callables,
        parameters: parameters.finish(),
        namespace,
        records,
    }
}
