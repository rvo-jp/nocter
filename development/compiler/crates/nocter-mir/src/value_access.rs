//! Closed private control, computation and lifecycle bodies; released admission stays separate.
mod computation;
mod lower;
mod repack;
#[cfg(test)]
mod tests;
mod validate;

use std::collections::BTreeMap;

use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessInstanceId, AccessLocalId, Arena, MirLocalId,
};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeDataId, RuntimeEnvironment, RuntimeValueSignature,
};
pub use nocter_target_program::value_access::CallTarget;
use nocter_target_program::value_access::{lifecycle, storage};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SlotKey {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Prepared {
        call: AccessExpressionId,
        index: usize,
    },
    Temporary {
        call: AccessExpressionId,
        index: usize,
    },
    Argument {
        call: AccessExpressionId,
        index: usize,
    },
    Operand {
        expression: AccessExpressionId,
        index: usize,
    },
    Predicate(AccessExpressionId),
    Answer(AccessExpressionId),
    FutureSelector(nocter_checking::value_access::flow::resources::Holder),
    FutureInput {
        owner: nocter_checking::value_access::flow::resources::Holder,
        index: usize,
        data: nocter_model::TypeId,
        operation: nocter_model::ValueInputOperation,
    },
    IndexSnapshot(nocter_checking::value_access::flow::resources::RetainedIndex),
    ArrayFrontier(nocter_checking::value_access::flow::resources::ArrayFrontier),
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Storage {
    Payload,
    Address,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub key: SlotKey,
    pub storage: Storage,
    pub representations: Box<[RuntimeDataId]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Projection {
    Field(nocter_model::ValueField),
    Environment,
    Index(MirLocalId),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Place {
    pub slot: MirLocalId,
    pub root: RuntimeDataId,
    pub projections: Box<[Projection]>,
    pub data: RuntimeDataId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposal {
    CallableAlternatives,
    Trivial,
    Error,
    Aggregate,
    Capture,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RemainderExclusion {
    Value(Place),
    ArrayPrefix { array: Place, endpoint: MirLocalId },
}
impl RemainderExclusion {
    #[must_use]
    pub const fn place(&self) -> &Place {
        match self {
            Self::Value(place) | Self::ArrayPrefix { array: place, .. } => place,
        }
    }
    #[must_use]
    pub const fn endpoint(&self) -> Option<MirLocalId> {
        match self {
            Self::Value(_) => None,
            Self::ArrayPrefix { endpoint, .. } => Some(*endpoint),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preservation {
    Complete,
    ArrayRemainder,
}

/// One unchanged subtree's physical transport, with sparse presence retained separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preserved {
    pub source: Place,
    pub destination: Place,
    pub kind: Preservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Record the statically known producer of a receiving activation.
    SetFutureSelector {
        destination: Place,
        tag: u64,
    },
    SelectedFutureMove {
        selector: Place,
        cases: Box<[FutureCaptureCase<FutureMoveInput>]>,
    },
    SelectedFutureCleanup {
        selector: Place,
        cases: Box<[FutureCaptureCase<FutureInputDisposal>]>,
    },
    /// Complete one wrapper construction at its checked commit phase.
    VariantTag {
        destination: Place,
        primary: bool,
    },
    /// The selected value computation initializes these bytes; this is not another constructor.
    Initialize(Place),
    Transfer {
        source: Place,
        destination: Place,
    },
    /// Move initialized disjoint siblings to their new offsets before a partial receipt.
    /// All preserved sources must be snapshotted before any destination is overwritten.
    Repack {
        source: Place,
        destination: Place,
        before: RuntimeDataId,
        /// Original value and prefix exclusions outside the replaced subtree.
        missing: Box<[RemainderExclusion]>,
        preserved: Box<[Preserved]>,
    },
    /// A distinct guarded wider representation, with separate source/destination payload layouts.
    /// Logical ownership and positive code/context identity were checked before this boundary.
    Transport {
        source: Place,
        destination: Place,
    },
    Reinterpret {
        source: Place,
        destination: Place,
    },
    Dispose {
        source: Place,
        disposal: Disposal,
    },
    DisposeArrayElements {
        source: Place,
        range: nocter_model::ArrayRange<MirLocalId>,
    },
    /// Exact sparse exclusions projected from the checked owner; ends the array after cleanup.
    DisposeArrayRemainder {
        source: Place,
        missing: Box<[RemainderExclusion]>,
    },
    /// Logical coverage consolidation; native storage needs no runtime ownership map.
    ConsolidateArrayPrefix {
        source: Place,
        endpoint: MirLocalId,
    },
    End(Place),
    BindAddress {
        destination: MirLocalId,
        source: Place,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Constant {
    EnumTag(u64),
    Integer(i128),
    Boolean(bool),
    Zero,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallInput {
    pub source: Place,
    pub operation: nocter_model::ValueInputOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub target: CallTarget,
    /// Frozen callee signature order, including its environment when present.
    pub inputs: Box<[CallInput]>,
    /// Filled on normal return before adapter cleanup or result receipt.
    pub answer: Place,
}

/// Complete static case identity paired by checking with its actual body specialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedCallee {
    pub data: RuntimeDataId,
    pub target: AccessInstanceId,
}
/// Invoke one retained read environment under the original carrier's stored selector.
/// Case environments cannot be projected outside this operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedCall {
    pub carrier: Place,
    pub cases: Box<[SelectedCallee]>,
    pub inputs: Box<[CallInput]>,
    pub answer: Place,
}

/// Drive the local activation identified by the selector, then stage its output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedDrive {
    pub selector: Place,
    pub cases: Box<[FutureDriveCase]>,
    pub answer: Place,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureDriveCase {
    pub tag: u64,
    pub target: AccessInstanceId,
    pub inputs: Box<[CallInput]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureCaptureCase<T> {
    pub tag: u64,
    pub fields: Box<[T]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureMoveInput {
    pub source: Place,
    pub destination: Place,
    pub operation: nocter_model::ValueInputOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureInputDisposal {
    pub source: Place,
    pub disposal: Disposal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// A terminal source bounds failure; no element payload or selected address exists.
    BoundsTrap,
    /// Observe the stored tag without consuming the carrier.
    PrimaryCondition {
        source: Place,
        destination: Place,
    },
    RequireVariant {
        source: Place,
        primary: bool,
    },
    Frozen {
        destination: Place,
        value: nocter_model::FrozenData,
    },
    Constant {
        destination: Place,
        value: Constant,
    },
    Copy {
        source: Place,
        destination: Place,
    },
    /// Check and capture one evaluated element address, or snapshot its selected Copy payload.
    Index {
        base: Place,
        index: Place,
        destination: Place,
        copy: bool,
        bounds: nocter_model::IndexBoundsCheck,
    },
    /// Copy the established seed into the homogeneous result; count belongs to its schema.
    Repeat {
        source: Place,
        destination: Place,
    },
    Storage(Effect),
    Unary {
        operation: crate::MirUnaryOperation,
        operand: Place,
        destination: Place,
        check: crate::MirArithmeticCheck,
    },
    Binary {
        operation: crate::MirBinaryOperation,
        left: Place,
        right: Place,
        destination: Place,
        invert: bool,
        check: crate::MirArithmeticCheck,
    },
    Call(Call),
    SelectedCall(SelectedCall),
    SelectedDrive(SelectedDrive),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(AccessFlowPointId),
    Branch {
        condition: Place,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    },
    Return(Place),
    Unreachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitValue {
    pub word: usize,
    pub mask: u64,
    pub value: bool,
}

#[derive(Clone, Debug)]
pub struct Alternative {
    pub tests: Box<[BitValue]>,
    pub before: Box<[Effect]>,
    pub before_writes: Box<[BitValue]>,
    pub after: Box<[Effect]>,
    pub after_writes: Box<[BitValue]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Location {
    pub point: AccessFlowPointId,
    pub action: usize,
}

#[derive(Clone, Debug)]
pub struct Action {
    /// Run after before-effects/writes and before entering the selected call.
    pub call_bindings: Box<[Effect]>,
    pub operations: Box<[Operation]>,
    pub alternatives: Box<[Alternative]>,
}

/// Independent physical obligations; neither supplies ownership or source correspondence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GeometryRequirement {
    Complete,
    /// Every retained logical endpoint keeps its offset within one common owning allocation.
    ProjectionOffsets(Box<[nocter_model::ValuePath<Option<u64>>]>),
}

/// Physical compatibility for an original scoped write or adapter exit, not a resource effect.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GeometryConstraint {
    pub at: Location,
    pub before: RuntimeDataId,
    pub after: RuntimeDataId,
    pub requirement: GeometryRequirement,
}

#[derive(Clone, Debug)]
pub struct Body {
    statics: Box<[(MirLocalId, nocter_model::ValueStatic)]>,
    signature: RuntimeValueSignature,
    slots: Arena<MirLocalId, Slot>,
    flags: Box<[u64]>,
    actions: BTreeMap<Location, Action>,
    entry: AccessFlowPointId,
    terminators: BTreeMap<AccessFlowPointId, Terminator>,
    inputs: Box<[MirLocalId]>,
    geometry: Box<[GeometryConstraint]>,
}
impl Body {
    #[must_use]
    pub fn geometry(&self) -> &[GeometryConstraint] {
        &self.geometry
    }
    #[must_use]
    pub fn statics(&self) -> &[(MirLocalId, nocter_model::ValueStatic)] {
        &self.statics
    }
    #[must_use]
    pub const fn entry(&self) -> AccessFlowPointId {
        self.entry
    }
    #[must_use]
    pub const fn terminators(&self) -> &BTreeMap<AccessFlowPointId, Terminator> {
        &self.terminators
    }
    #[must_use]
    pub fn inputs(&self) -> &[MirLocalId] {
        &self.inputs
    }
    #[must_use]
    pub const fn signature(&self) -> &RuntimeValueSignature {
        &self.signature
    }
    #[must_use]
    pub const fn slots(&self) -> &Arena<MirLocalId, Slot> {
        &self.slots
    }
    #[must_use]
    pub fn flags(&self) -> &[u64] {
        &self.flags
    }
    #[must_use]
    pub const fn actions(&self) -> &BTreeMap<Location, Action> {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Storage(storage::Failure),
    InvalidBody(AccessInstanceId),
}

/// One immutable global object with its exact closed runtime data.
#[derive(Clone, Debug)]
pub struct Static {
    pub data: RuntimeDataId,
    pub value: nocter_model::FrozenData,
}
/// Closed private MIR bodies and global data, without a released-program adapter.
#[derive(Clone, Debug)]
pub struct Program {
    statics: BTreeMap<nocter_model::ValueStatic, Static>,
    runtime: RuntimeEnvironment,
    bodies: BTreeMap<AccessInstanceId, Body>,
    root: AccessInstanceId,
}
impl Program {
    /// Freeze computation, control and lifecycle from one exact selected instance program.
    /// # Errors
    /// Rejects missing identities, inconsistent slot roles or unresolved concrete data.
    pub fn lower(program: &lifecycle::Program, abi: RuntimeAbiIdentity) -> Result<Self, Failure> {
        let storage =
            storage::prepare(program.instance_program(), abi, program.payload_instances())
                .map_err(Failure::Storage)?;
        let bodies: BTreeMap<AccessInstanceId, Body> = program
            .bodies()
            .iter()
            .map(|(&id, body)| {
                Ok((
                    id,
                    lower::body(program.instance_program(), &storage, id, body)?,
                ))
            })
            .collect::<Result<_, Failure>>()?;
        let mut statics = BTreeMap::new();
        for body in bodies.values() {
            for &(slot, id) in &body.statics {
                let source = program.instance_program().unit();
                let data = *body
                    .slots
                    .get(slot)
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?
                    .representations
                    .first()
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?;
                let value = source
                    .static_value(id)
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?
                    .clone();
                if let Some(existing) = statics.insert(id, Static { data, value })
                    && existing.data != data
                {
                    return Err(Failure::InvalidBody(program.instance_program().root()));
                }
            }
        }
        let result = Self {
            statics,
            runtime: storage.into_environment(),
            bodies,
            root: program.instance_program().root(),
        };
        validate::program(&result)?;
        Ok(result)
    }
    #[must_use]
    pub const fn statics(&self) -> &BTreeMap<nocter_model::ValueStatic, Static> {
        &self.statics
    }
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeEnvironment {
        &self.runtime
    }
    #[must_use]
    pub const fn bodies(&self) -> &BTreeMap<AccessInstanceId, Body> {
        &self.bodies
    }
    #[must_use]
    pub const fn root(&self) -> AccessInstanceId {
        self.root
    }
}
