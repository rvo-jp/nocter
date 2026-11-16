//! Closed data shapes for storage, independent of access operations and physical placement.
use nocter_declarations::value_access::{DataInstanceKey, Program};
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
    /// Resolve an already materialized contextual payload without rebuilding its caller frame.
    /// # Errors
    /// Rejects unresolved or absent positive data identities.
    pub fn instance(
        &self,
        program: &Program,
        key: &DataInstanceKey,
    ) -> Result<RuntimeDataId, Failure> {
        let invalid = || Failure::InvalidPayload(key.data());
        let key = shapes::representation_key(program, key.clone())?;
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
        let payload = instance.input(input.local).ok_or_else(invalid)?;
        Ok(RuntimeValueInput {
            local: input.local,
            data: storage.instance(instances.unit().program(), payload.instance())?,
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
    let payload = instance.result().payload().ok_or_else(invalid)?;
    let result = storage.instance(instances.unit().program(), payload.instance())?;
    Ok(RuntimeValueSignature::new(environment, explicit, result))
}

/// Projects explicit canonical data domains without constructing an executable instance graph.
/// Stored shapes grant no active selector, source admission, ownership or invocation permission.
/// # Errors
/// Rejects unresolved, unsupported or inconsistent storage graphs without publishing a partial table.
pub fn prepare_data(
    program: &Program,
    abi: RuntimeAbiIdentity,
    data: impl IntoIterator<Item = DataInstanceKey>,
) -> Result<Prepared, Failure> {
    let mut shapes = shapes::Builder::new(program);
    for key in data {
        shapes.intern(key)?;
    }
    shapes.finish(abi)
}

/// Projects instance data and explicitly requested actual payload keys through one interner.
/// Code selection, source access and lifecycle permission remain upstream.
///
/// # Errors
/// Rejects an unresolved shape or inconsistent closed runtime environment. No partial result is
/// published. The selected ABI supplies representation facts, not executable target admission.
pub fn prepare(
    instances: &InstanceProgram,
    abi: RuntimeAbiIdentity,
    additional: impl IntoIterator<Item = DataInstanceKey>,
) -> Result<Prepared, Failure> {
    let program = instances.unit().program();
    let mut shapes = shapes::Builder::new(program);
    for (_, instance) in instances.instances().iter() {
        for payload in instance.payloads() {
            shapes.intern(payload.instance().clone())?;
        }
        for key in instance.values.roots() {
            shapes.intern(key.clone())?;
        }
        for key in instance.metadata() {
            shapes.intern(key.clone())?;
        }
    }
    for key in additional {
        shapes.intern(key)?;
    }
    shapes.finish(abi)
}
