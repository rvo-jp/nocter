//! Closed static instances of the private value-binding product. No ABI or native admission.
mod build;
mod invocation;
pub mod lifecycle;
mod projection;
pub mod storage;
#[cfg(test)]
mod tests;

pub use invocation::{CallTarget, Environment, EnvironmentExit, InputBinding, Invocation};
use nocter_checking::value_access::{accepted::Unit, flow};
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessInstanceId, AccessLocalId, AccessParameterId, Arena,
    TypeId,
};
use std::{collections::BTreeMap, sync::Arc};

pub use nocter_declarations::value_access::{CodeInstanceKey as InstanceKey, ConcreteWitness};

pub use nocter_checking::value_access::flow::resources::InstantiatedPayload as Payload;
pub type DataEvidence = nocter_checking::value_access::accepted::DataEvidence<Payload>;

#[derive(Clone, Debug)]
pub struct Instance {
    key: InstanceKey,
    calls: BTreeMap<AccessExpressionId, Invocation>,
    data: BTreeMap<TypeId, DataEvidence>,
    inputs: BTreeMap<AccessLocalId, Payload>,
    result: DataEvidence,
    prepared: BTreeMap<(AccessExpressionId, usize), Payload>,
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
    /// Actual body input data keyed only by the original input local, without another input layout.
    #[must_use]
    pub fn input(&self, local: AccessLocalId) -> Option<&Payload> {
        self.inputs.get(&local)
    }
    /// Original result metadata; an unproduced contract grants no payload.
    #[must_use]
    pub const fn result(&self) -> &DataEvidence {
        &self.result
    }
    /// Actual prepared argument metadata, including preparations whose call never commits.
    #[must_use]
    pub fn prepared(&self, call: AccessExpressionId, index: usize) -> Option<&Payload> {
        self.prepared.get(&(call, index))
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
    Invocation {
        site: nocter_model::ValueApplication,
        failure: nocter_checking::value_access::InstantiationFailure,
    },
    Input {
        code: nocter_model::ValueCode,
        local: AccessLocalId,
        failure: nocter_checking::value_access::InstantiationFailure,
    },
    Result {
        code: nocter_model::ValueCode,
        failure: nocter_checking::value_access::InstantiationFailure,
    },
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
    /// and metadata-work exhaustion reject; no partially closed graph is published.
    pub fn prepare(unit: Arc<Unit>, root: AccessCodeId) -> Result<Self, Failure> {
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            nocter_constant_evaluation::CompileTimeEvaluationLimits::default().steps(),
        );
        Self::prepare_with_budget(unit, root, &mut work)
    }
    /// Close reached invocations under one shared deterministic metadata-work quota.
    /// # Errors
    /// Preserves root/integrity failures and reports the exact failing invocation. No partial
    /// instance graph is published, and the accepted Unit remains reusable after failure.
    pub fn prepare_with_budget(
        unit: Arc<Unit>,
        root: AccessCodeId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<Self, Failure> {
        build::close(unit, root, work)
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
