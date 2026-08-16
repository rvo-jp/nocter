use nocter_model::{
    BodyNodeId, BorrowCapability, CallableCapability, CallableId, CaptureId, FieldId,
    LocalBindingId, LoopId, NominalTypeId, PlaceId, TypeId, VariantId,
};

use crate::expected::OutcomeLayer;

use super::{GenericArguments, StaticDispatch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNode {
    ty: TypeId,
    operation: CheckedOperation,
}

impl CheckedNode {
    pub(super) const fn new(ty: TypeId, operation: CheckedOperation) -> Self {
        Self { ty, operation }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedOperation {
    Complete,
    Constant(ConstantValue),
    Place(PlaceId),
    Copy(PlaceId),
    Move(PlaceId),
    Borrow {
        capability: BorrowCapability,
        place: PlaceId,
    },
    Call(CheckedCall),
    Coerce {
        value: BodyNodeId,
        target: TypeId,
        dispatch: StaticDispatch,
    },
    Primitive(PrimitiveOperation),
    Aggregate(AggregateConstruction),
    Outcome(CheckedOutcome),
    Closure(CheckedClosure),
    Sequence(CheckedSequence),
    StringLiteral {
        constructor: CallableId,
        text: Box<str>,
        allocation: AllocationSelection,
    },
    Interpolation(CheckedInterpolation),
    Control(CheckedControl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    Integer(u64),
    Text(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Static(StaticDispatch),
    CallableValue {
        value: BodyNodeId,
        capability: CallableCapability,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCall {
    target: CallTarget,
    receiver: Option<BodyNodeId>,
    arguments: Box<[BodyNodeId]>,
    generic_arguments: GenericArguments,
}

impl CheckedCall {
    #[must_use]
    pub const fn target(&self) -> &CallTarget {
        &self.target
    }

    #[must_use]
    pub const fn receiver(&self) -> Option<BodyNodeId> {
        self.receiver
    }

    #[must_use]
    pub const fn arguments(&self) -> &[BodyNodeId] {
        &self.arguments
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrimitiveUnary {
    LogicalNot,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrimitiveBinary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    Equal,
    Less,
    LogicalAnd,
    LogicalOr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrimitiveOperation {
    Unary {
        operation: PrimitiveUnary,
        operand: BodyNodeId,
    },
    Binary {
        operation: PrimitiveBinary,
        left: BodyNodeId,
        right: BodyNodeId,
    },
    IntegerConversion {
        operand: BodyNodeId,
        target: TypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AggregateConstruction {
    Struct {
        definition: NominalTypeId,
        fields: Box<[(FieldId, BodyNodeId)]>,
    },
    Enum {
        variant: VariantId,
        payload: Box<[BodyNodeId]>,
    },
    FixedArray(Box<[BodyNodeId]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedOutcome {
    Inject {
        layer: OutcomeLayer,
        payload: BodyNodeId,
    },
    Absent,
    Failure(BodyNodeId),
    Propagate {
        operand: BodyNodeId,
        layer: OutcomeLayer,
    },
    Recover {
        operand: BodyNodeId,
        layer: OutcomeLayer,
        binding: Option<LocalBindingId>,
        fallback: BodyNodeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedClosure {
    capability: CallableCapability,
    parameters: Box<[LocalBindingId]>,
    captures: Box<[CaptureId]>,
    body: BodyNodeId,
}

impl CheckedClosure {
    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn parameters(&self) -> &[LocalBindingId] {
        &self.parameters
    }

    #[must_use]
    pub const fn captures(&self) -> &[CaptureId] {
        &self.captures
    }

    #[must_use]
    pub const fn body(&self) -> BodyNodeId {
        self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AllocationSelection {
    CurrentRegion,
    Explicit(BodyNodeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedIteration {
    source: BodyNodeId,
    acquisition: IterationAcquisition,
    next: StaticDispatch,
    item: TypeId,
}

impl TypedIteration {
    #[must_use]
    pub const fn source(&self) -> BodyNodeId {
        self.source
    }

    #[must_use]
    pub const fn acquisition(&self) -> &IterationAcquisition {
        &self.acquisition
    }

    #[must_use]
    pub const fn next(&self) -> StaticDispatch {
        self.next
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.item
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IterationAcquisition {
    Direct,
    Expansion(StaticDispatch),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpreadMode {
    Copy,
    Borrow,
    Move,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SequenceElement {
    Value(BodyNodeId),
    Spread {
        mode: SpreadMode,
        iteration: TypedIteration,
        exact_size: StaticDispatch,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedSequence {
    constructor: CallableId,
    elements: Box<[SequenceElement]>,
    allocation: AllocationSelection,
}

impl CheckedSequence {
    #[must_use]
    pub const fn constructor(&self) -> CallableId {
        self.constructor
    }

    #[must_use]
    pub const fn elements(&self) -> &[SequenceElement] {
        &self.elements
    }

    #[must_use]
    pub const fn allocation(&self) -> AllocationSelection {
        self.allocation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterpolationPart {
    Text(Box<str>),
    Formatted {
        value: BodyNodeId,
        formatter: StaticDispatch,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInterpolation {
    parts: Box<[InterpolationPart]>,
    allocation: AllocationSelection,
}

impl CheckedInterpolation {
    #[must_use]
    pub const fn parts(&self) -> &[InterpolationPart] {
        &self.parts
    }

    #[must_use]
    pub const fn allocation(&self) -> AllocationSelection {
        self.allocation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedPattern {
    Variant {
        variant: VariantId,
        payload: Box<[LocalBindingId]>,
    },
    OptionalPresent(LocalBindingId),
    OptionalAbsent,
    FallibleSuccess(LocalBindingId),
    FallibleFailure(LocalBindingId),
    Wildcard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArm {
    pattern: CheckedPattern,
    body: BodyNodeId,
}

impl CheckedMatchArm {
    #[must_use]
    pub const fn pattern(&self) -> &CheckedPattern {
        &self.pattern
    }

    #[must_use]
    pub const fn body(&self) -> BodyNodeId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopKind {
    Infinite,
    While {
        condition: BodyNodeId,
    },
    For {
        binding: LocalBindingId,
        iteration: TypedIteration,
    },
    Range {
        binding: LocalBindingId,
        start: BodyNodeId,
        end: BodyNodeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLoop {
    kind: LoopKind,
    body: BodyNodeId,
}

impl CheckedLoop {
    #[must_use]
    pub const fn kind(&self) -> &LoopKind {
        &self.kind
    }

    #[must_use]
    pub const fn body(&self) -> BodyNodeId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedControl {
    Block {
        statements: Box<[BodyNodeId]>,
        result: Option<BodyNodeId>,
    },
    Bind {
        binding: LocalBindingId,
        initializer: BodyNodeId,
    },
    Assign {
        target: PlaceId,
        value: BodyNodeId,
    },
    CompoundAssign {
        target: PlaceId,
        value: BodyNodeId,
        operation: PrimitiveBinary,
    },
    Discard(BodyNodeId),
    Return(Option<BodyNodeId>),
    Break(LoopId),
    Continue(LoopId),
    Drop(PlaceId),
    If {
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
    },
    Match {
        subject: BodyNodeId,
        arms: Box<[CheckedMatchArm]>,
    },
    Loop(LoopId),
    Region {
        binding: LocalBindingId,
        allocator: BodyNodeId,
        body: BodyNodeId,
    },
}
