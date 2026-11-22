//! Closed static instances of the private value-binding product. No ABI or native admission.
mod build;
mod entry;
pub use entry::{entry_result, executable_abi};
mod invocation;
pub mod lifecycle;
mod projection;
pub mod storage;
#[cfg(test)]
mod tests;

pub use invocation::{
    CallTarget, Environment, EnvironmentExit, InputBinding, Invocation, InvocationCase,
};
use nocter_checking::value_access::{accepted::Unit, flow};
use nocter_declarations::value_access::DataInstanceKey;
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessInstanceId, AccessLocalId, AccessParameterId, Arena,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub use nocter_declarations::value_access::{CodeInstanceKey as InstanceKey, ConcreteWitness};

pub use nocter_checking::value_access::flow::resources::InstantiatedPayload as Payload;
pub type DataEvidence = nocter_checking::value_access::accepted::DataEvidence<Payload>;

/// An empty activation can drive a statically selected body or dispatch its stored producer tag.
#[derive(Clone, Debug)]
pub enum Drive {
    Direct(AccessInstanceId),
    Tagged(Box<[(u64, AccessInstanceId)]>),
}

impl Drive {
    /// Runtime scalar used for local empty activation selectors.
    pub const SELECTOR_TYPE: nocter_model::BuiltinType = nocter_model::BuiltinType::U64;

    pub(super) fn targets(&self) -> impl Iterator<Item = AccessInstanceId> + '_ {
        let (direct, tagged) = match self {
            Self::Direct(target) => (Some(*target), &[][..]),
            Self::Tagged(cases) => (None, cases.as_ref()),
        };
        direct
            .into_iter()
            .chain(tagged.iter().map(|(_, target)| *target))
    }
}

#[derive(Clone, Debug)]
pub struct Instance {
    calls: BTreeMap<AccessExpressionId, Invocation>,
    drives: BTreeMap<flow::Location, Drive>,
    metadata: BTreeSet<DataInstanceKey>,
    inputs: BTreeMap<AccessLocalId, Payload>,
    result: DataEvidence,
    prepared: BTreeMap<(AccessExpressionId, usize), Payload>,
    values: projection::Values,
    queries: nocter_checking::value_access::accepted::InstanceMetadata,
}
impl Instance {
    #[must_use]
    pub const fn control(&self) -> &nocter_checking::value_access::InstanceControl {
        self.queries.control()
    }
    #[must_use]
    pub const fn key(&self) -> &InstanceKey {
        self.control().frame()
    }
    /// Only reachable invocation commits have an instance edge. Value references alone do not.
    #[must_use]
    pub const fn calls(&self) -> &BTreeMap<AccessExpressionId, Invocation> {
        &self.calls
    }
    /// Exact executable targets of a reached, qualified local drive.
    #[must_use]
    pub fn drive(&self, at: flow::Location) -> Option<&Drive> {
        self.drives.get(&at)
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
    /// Exact original operand data; metadata does not grant a value or access permission.
    #[must_use]
    pub fn operand(&self, at: flow::Location, expression: AccessExpressionId) -> Option<&Payload> {
        self.values.operand(at, expression)
    }
    /// Final stored source data after an original action, independent of resource phases.
    #[must_use]
    pub fn definition(
        &self,
        at: flow::Location,
        slot: nocter_checking::value_access::ValueSourceSlot,
    ) -> Option<&Payload> {
        self.values.definition(at, slot)
    }
    /// Receiving domain selected by an original expression merge; actual writes own availability.
    #[must_use]
    pub fn receipt(
        &self,
        expression: AccessExpressionId,
    ) -> Option<&nocter_declarations::value_access::DataInstanceKey> {
        self.values.receipt(expression)
    }
    /// Original reference-root contract metadata; this is not a resource-phase or layout proof.
    #[must_use]
    pub fn referent_root(
        &self,
        at: flow::Location,
        referent: nocter_checking::value_access::Referent,
    ) -> Option<&DataInstanceKey> {
        self.values.root(at, referent)
    }
    /// Original normal-completion root contract, without a resource-phase or mutation proof.
    #[must_use]
    pub fn referent_root_after(
        &self,
        at: flow::Location,
        referent: nocter_checking::value_access::Referent,
    ) -> Option<&DataInstanceKey> {
        self.values.root_after(at, referent)
    }
    /// Independently closed metadata roots, never a contextual value or source-availability map.
    #[must_use]
    pub const fn metadata(&self) -> &BTreeSet<DataInstanceKey> {
        &self.metadata
    }
    /// Original value/channel metadata, including interrupted preparation prefixes.
    pub fn payloads(&self) -> impl Iterator<Item = &Payload> {
        self.values
            .payloads()
            .chain(self.prepared.values())
            .chain(self.inputs.values())
            .chain(match &self.result {
                DataEvidence::Payload(payload) => Some(payload),
                DataEvidence::Unproduced => None,
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Control {
        code: nocter_model::ValueCode,
        failure: nocter_checking::value_access::InstantiationFailure,
    },
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
    Value {
        code: nocter_model::ValueCode,
        at: flow::Location,
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
            .code_body(self.instances.get(instance)?.key().code())
    }
}
