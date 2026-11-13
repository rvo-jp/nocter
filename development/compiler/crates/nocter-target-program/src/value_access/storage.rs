//! Closed data shapes for storage, independent of access operations and physical placement.
use nocter_checking::value_access::accepted::DataEvidence;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, Program};
use nocter_model::{AccessInstanceId, TypeId};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeDataId, RuntimeEnvironment, RuntimeEnvironmentError,
    RuntimeTypeTableBuildError, RuntimeValueInput, RuntimeValueSignature,
};
use std::collections::BTreeMap;
mod shapes;

use super::InstanceProgram;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidInstance(AccessInstanceId),
    InvalidPayload(TypeId),
    Types(RuntimeTypeTableBuildError),
    Environment(RuntimeEnvironmentError),
}

/// One closed native shape table and its contextual identity interner.
/// Its keys select representations; they grant no source access, payload or cleanup proof.
#[derive(Clone, Debug)]
pub struct Prepared {
    runtime: RuntimeEnvironment,
    ids: BTreeMap<DataInstanceKey, RuntimeDataId>,
}
impl Prepared {
    #[must_use]
    pub const fn environment(&self) -> &RuntimeEnvironment {
        &self.runtime
    }
    #[must_use]
    pub fn into_environment(self) -> RuntimeEnvironment {
        self.runtime
    }
    /// Resolve only an already materialized shape through the canonical declaration authority.
    /// # Errors
    /// Rejects unresolved bindings and absent positive storage; empty metadata is not a payload.
    pub fn data(
        &self,
        program: &Program,
        frame: &CodeInstanceKey,
        source: TypeId,
    ) -> Result<RuntimeDataId, Failure> {
        let invalid = || Failure::InvalidPayload(source);
        let key = program
            .instantiate_data(source, frame)
            .map_err(|_| invalid())?;
        let key = key
            .function_representation(program)
            .map_err(|_| invalid())?
            .unwrap_or(key);
        self.ids.get(&key).copied().ok_or_else(invalid)
    }
}

/// Projects frozen body input slots through the same instance data substitution as storage.
///
/// # Errors
/// Rejects absent instance/body/input or unresolved concrete input/result data.
pub fn signature(
    instances: &InstanceProgram,
    storage: &Prepared,
    id: AccessInstanceId,
) -> Result<RuntimeValueSignature, Failure> {
    let invalid = || Failure::InvalidInstance(id);
    let instance = instances.instances().get(id).ok_or_else(invalid)?;
    let body = instances.body(id).ok_or_else(invalid)?.typed();
    let inputs = body.body().inputs();
    let project = |input: nocter_checking::value_access::BodyInput| {
        let local = body.body().locals().get(input.local).ok_or_else(invalid)?;
        instance
            .data()
            .get(&local.data)
            .and_then(DataEvidence::payload)
            .ok_or_else(invalid)?;
        Ok(RuntimeValueInput {
            local: input.local,
            data: storage.data(instances.unit().program(), instance.key(), local.data)?,
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
    instance
        .data()
        .get(&body.result_data())
        .and_then(DataEvidence::payload)
        .ok_or_else(invalid)?;
    let result = storage.data(
        instances.unit().program(),
        instance.key(),
        body.result_data(),
    )?;
    Ok(RuntimeValueSignature::new(environment, explicit, result))
}

/// Projects produced concrete data only. Code selection and access proof remain upstream.
///
/// # Errors
/// Rejects an unresolved shape or inconsistent closed runtime environment. No partial result is
/// published. The selected ABI supplies representation facts, not executable target admission.
pub fn prepare(instances: &InstanceProgram, abi: RuntimeAbiIdentity) -> Result<Prepared, Failure> {
    let program = instances.unit().program();
    let mut shapes = shapes::Builder::new(program);
    for (_, instance) in instances.instances().iter() {
        for (&source, evidence) in instance.data() {
            if evidence.payload().is_none() {
                continue;
            }
            let data = program
                .instantiate_data(source, instance.key())
                .map_err(|_| Failure::InvalidPayload(source))?;
            shapes.intern(data)?;
        }
    }
    shapes.finish(abi)
}
