//! Closed data shapes for storage, independent of access operations and physical placement.
use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::value_access::{accepted::DataEvidence, flow::resources::Payload};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessInstanceId, BuiltinType, TypeId, ValueFunctionOrigin};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeEnvironment, RuntimeEnvironmentError, RuntimePrimitive, RuntimeType,
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeTypeTableBuildError,
    RuntimeTypeTableBuilder, RuntimeValueFieldRepresentation, RuntimeValueInput,
    RuntimeValueSignature,
};

use super::InstanceProgram;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidInstance(AccessInstanceId),
    InvalidPayload(TypeId),
    Types(RuntimeTypeTableBuildError),
    Environment(RuntimeEnvironmentError),
}

/// Projects frozen body input slots through the same instance data substitution as storage.
///
/// # Errors
/// Rejects absent instance/body/input or unresolved concrete input/result data.
pub fn signature(
    instances: &InstanceProgram,
    id: AccessInstanceId,
) -> Result<RuntimeValueSignature, Failure> {
    let invalid = || Failure::InvalidInstance(id);
    let instance = instances.instances().get(id).ok_or_else(invalid)?;
    let body = instances.body(id).ok_or_else(invalid)?.typed();
    let inputs = body.body().inputs();
    let project = |input: nocter_checking::value_access::BodyInput| {
        let local = body.body().locals().get(input.local).ok_or_else(invalid)?;
        let payload = instance
            .data()
            .get(&local.data)
            .and_then(DataEvidence::payload)
            .ok_or_else(invalid)?;
        Ok(RuntimeValueInput {
            local: input.local,
            data: payload.data(),
            operation: input.operation,
        })
    };
    let environment = inputs.environment().map(project).transpose()?;
    let explicit = inputs
        .explicit()
        .iter()
        .copied()
        .map(project)
        .collect::<Result<Box<[_]>, Failure>>()?;
    let result = instance
        .data()
        .get(&body.result_data())
        .ok_or_else(invalid)?
        .payload()
        .ok_or_else(invalid)?
        .data();
    Ok(RuntimeValueSignature::new(environment, explicit, result))
}

/// Projects produced concrete data only. Code selection and access proof remain upstream.
///
/// # Errors
/// Rejects an unresolved shape or inconsistent closed runtime environment. No partial result is
/// published. The selected ABI supplies representation facts, not executable target admission.
pub fn prepare(
    instances: &InstanceProgram,
    abi: RuntimeAbiIdentity,
) -> Result<RuntimeEnvironment, Failure> {
    let program = instances.unit().program();
    let mut pending: BTreeSet<_> = instances
        .instances()
        .iter()
        .flat_map(|(_, instance)| instance.data().values())
        .filter_map(|evidence| evidence.payload().map(Payload::data))
        .collect();
    let mut visited = BTreeSet::new();
    let mut types = RuntimeTypeTableBuilder::new();
    let mut representations = BTreeMap::new();
    while let Some(data) = pending.pop_first() {
        if !visited.insert(data) {
            continue;
        }
        let invalid = || Failure::InvalidPayload(data);
        let kind = match program.data_schema(data).ok_or_else(invalid)? {
            PayloadSchema::Array(array) => {
                if array.is_empty() {
                    RuntimeType::EmptyArray {
                        element: array.element(),
                    }
                } else {
                    pending.insert(array.element());
                    // Native homogeneous traversal uses canonical compiler scalar identities.
                    for builtin in [BuiltinType::Usize, BuiltinType::Bool] {
                        pending.insert(program.data_type(builtin).ok_or_else(invalid)?);
                    }
                    RuntimeType::FixedArray {
                        element: array.element(),
                        length: array.length(),
                    }
                }
            }
            PayloadSchema::Optional(payload) => {
                pending.insert(payload);
                pending.insert(program.data_type(BuiltinType::Bool).ok_or_else(invalid)?);
                pending.insert(program.data_type(BuiltinType::U8).ok_or_else(invalid)?);
                RuntimeType::Optional(payload)
            }
            PayloadSchema::Builtin(builtin) => match builtin {
                BuiltinType::Bool => RuntimeType::Primitive(RuntimePrimitive::Bool),
                _ if nocter_model::BuiltinNumericType::for_builtin(builtin)
                    .and_then(nocter_model::BuiltinNumericType::integer_bounds)
                    .is_some() =>
                {
                    RuntimeType::Primitive(RuntimePrimitive::for_builtin(builtin))
                }
                BuiltinType::Error => RuntimeType::Primitive(RuntimePrimitive::Error),
                BuiltinType::Void => RuntimeType::Unit,
                _ => return Err(invalid()),
            },
            PayloadSchema::Aggregate(aggregate) => {
                let fields = aggregate
                    .components()
                    .map(|component| {
                        pending.insert(component.data);
                        RuntimeValueFieldRepresentation::new(component.field, component.data)
                    })
                    .collect();
                representations.insert(data, RuntimeTypeRepresentation::ValueAggregate { fields });
                RuntimeType::Aggregate
            }
            PayloadSchema::Function(witness) => {
                let environment = match witness.origin {
                    ValueFunctionOrigin::Declaration(_) => None,
                    ValueFunctionOrigin::Closure(id) => {
                        Some(program.closure(id).ok_or_else(invalid)?.environment())
                    }
                    ValueFunctionOrigin::Parameter(_)
                    | ValueFunctionOrigin::Result(_)
                    | ValueFunctionOrigin::Application(_) => return Err(invalid()),
                };
                pending.extend(environment);
                RuntimeType::StaticCallable { environment }
            }
        };
        types.insert(data, kind).map_err(Failure::Types)?;
    }
    RuntimeEnvironment::new(
        types.finish().map_err(Failure::Types)?,
        RuntimeTypeRepresentationTable::new(representations),
        abi,
    )
    .map_err(Failure::Environment)
}
