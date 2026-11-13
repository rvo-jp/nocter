//! Closed static instances of the private value-binding product. No ABI or native admission.
mod build;
mod invocation;
pub mod lifecycle;
mod projection;
pub mod storage;
#[cfg(test)]
mod tests;

pub use invocation::{CallTarget, Environment, EnvironmentExit, InputBinding, Invocation};
use nocter_checking::value_access::{
    accepted::{DataEvidence, Unit},
    flow,
};
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessInstanceId, AccessParameterId, Arena, TypeId,
};
use std::{collections::BTreeMap, sync::Arc};

pub use nocter_declarations::value_access::{CodeInstanceKey as InstanceKey, ConcreteWitness};

#[derive(Clone, Debug)]
pub struct Instance {
    key: InstanceKey,
    calls: BTreeMap<AccessExpressionId, Invocation>,
    data: BTreeMap<TypeId, DataEvidence>,
}
impl Instance {
    #[must_use]
    pub const fn key(&self) -> &InstanceKey {
        &self.key
    }
    /// Only reachable invocation commits have an instance edge. Value references alone do not.
    #[must_use]
    pub const fn calls(&self) -> &BTreeMap<AccessExpressionId, Invocation> {
        &self.calls
    }
    /// Source data identities map to concrete payloads or checked unproduced structural contracts.
    #[must_use]
    pub const fn data(&self) -> &BTreeMap<TypeId, DataEvidence> {
        &self.data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    UnknownRoot(AccessCodeId),
    InitializerRoot(AccessCodeId),
    BodylessRoot(AccessCodeId),
    UnboundRoot(AccessParameterId),
    InvalidAcceptedUnit,
}

/// One root's finite static instance graph, paired with the exact accepted body/type authority.
/// This is not an executable layout, target capability or cleanup-lowering certificate.
#[derive(Clone, Debug)]
pub struct InstanceProgram {
    unit: Arc<Unit>,
    root: AccessInstanceId,
    instances: Arena<AccessInstanceId, Instance>,
}
impl InstanceProgram {
    /// Substitute already selected witness edges without source access or body rechecking.
    /// # Errors
    /// A root must exist and need no externally supplied callback witness. Invalid checked edges
    /// are integrity errors; no partially closed graph is published.
    pub fn prepare(unit: Arc<Unit>, root: AccessCodeId) -> Result<Self, Failure> {
        build::close(unit, root)
    }
    #[must_use]
    pub fn unit(&self) -> &Unit {
        &self.unit
    }
    #[must_use]
    pub const fn root(&self) -> AccessInstanceId {
        self.root
    }
    #[must_use]
    pub const fn instances(&self) -> &Arena<AccessInstanceId, Instance> {
        &self.instances
    }
    /// Pair each instance with its original checked body. Selected operations remain unchanged.
    #[must_use]
    pub fn body(&self, instance: AccessInstanceId) -> Option<&flow::Query> {
        self.unit
            .code_body(self.instances.get(instance)?.key.code())
    }
}
