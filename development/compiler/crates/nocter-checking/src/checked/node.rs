use nocter_model::{
    BodyNodeId, BodyScopeId, BorrowCapability, CallableCapability, CallableId, CaptureId, FieldId,
    LocalBindingId, LoopId, NominalTypeId, PlaceId, TypeId, VariantId,
};

use crate::expected::OutcomeLayer;

use super::StaticSelection;

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
    BorrowConversion(CheckedBorrowConversion),
    Comparison(CheckedComparison),
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BorrowConversionPreparation {
    PreserveReadonly,
    PreserveReadwrite,
    WeakenReadwrite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BorrowConversionImplementation {
    CapabilityWeakening,
    Selected(StaticSelection),
}

/// One complete expected-type borrow conversion selected before ownership and lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedBorrowConversion {
    value: BodyNodeId,
    target: TypeId,
    preparation: BorrowConversionPreparation,
    implementation: BorrowConversionImplementation,
}

impl CheckedBorrowConversion {
    pub(crate) const fn new(
        value: BodyNodeId,
        target: TypeId,
        preparation: BorrowConversionPreparation,
        implementation: BorrowConversionImplementation,
    ) -> Self {
        Self {
            value,
            target,
            preparation,
            implementation,
        }
    }

    #[must_use]
    pub const fn value(&self) -> BodyNodeId {
        self.value
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn preparation(&self) -> BorrowConversionPreparation {
        self.preparation
    }

    #[must_use]
    pub const fn implementation(&self) -> &BorrowConversionImplementation {
        &self.implementation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    Integer(i128),
    Text(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Static(StaticSelection),
    CallableValue {
        value: BodyNodeId,
        capability: CallableCapability,
        dispatch: StaticSelection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCall {
    target: CallTarget,
    receiver: Option<BodyNodeId>,
    arguments: Box<[BodyNodeId]>,
}

impl CheckedCall {
    pub(crate) fn new(
        target: CallTarget,
        receiver: Option<BodyNodeId>,
        arguments: impl Into<Box<[BodyNodeId]>>,
    ) -> Self {
        Self {
            target,
            receiver,
            arguments: arguments.into(),
        }
    }

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
    ShiftRightSigned,
    ShiftRightUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ComparisonOperation {
    Equal,
    Less,
}

/// How one source operand becomes the readonly logical value consumed by a comparison.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadonlyOperandPreparation {
    BorrowPlace,
    BorrowTemporary,
    UseReadonlyBorrow,
    WeakenReadwriteBorrow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedComparisonOperand {
    value: BodyNodeId,
    preparation: ReadonlyOperandPreparation,
    coercion: Option<StaticSelection>,
}

impl CheckedComparisonOperand {
    pub(crate) const fn new(
        value: BodyNodeId,
        preparation: ReadonlyOperandPreparation,
        coercion: Option<StaticSelection>,
    ) -> Self {
        Self {
            value,
            preparation,
            coercion,
        }
    }

    #[must_use]
    pub const fn value(&self) -> BodyNodeId {
        self.value
    }

    #[must_use]
    pub const fn preparation(&self) -> ReadonlyOperandPreparation {
        self.preparation
    }

    #[must_use]
    pub const fn coercion(&self) -> Option<&StaticSelection> {
        self.coercion.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComparisonImplementation {
    Primitive,
    Selected(StaticSelection),
    Unreachable,
}

/// One complete comparison plan with source evaluation and semantic invocation kept separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedComparison {
    operation: ComparisonOperation,
    left: CheckedComparisonOperand,
    right: CheckedComparisonOperand,
    implementation: ComparisonImplementation,
    reverse: bool,
    negate: bool,
}

impl CheckedComparison {
    pub(crate) const fn new(
        operation: ComparisonOperation,
        left: CheckedComparisonOperand,
        right: CheckedComparisonOperand,
        implementation: ComparisonImplementation,
        reverse: bool,
        negate: bool,
    ) -> Self {
        Self {
            operation,
            left,
            right,
            implementation,
            reverse,
            negate,
        }
    }

    #[must_use]
    pub const fn operation(&self) -> ComparisonOperation {
        self.operation
    }

    #[must_use]
    pub const fn left(&self) -> &CheckedComparisonOperand {
        &self.left
    }

    #[must_use]
    pub const fn right(&self) -> &CheckedComparisonOperand {
        &self.right
    }

    #[must_use]
    pub const fn implementation(&self) -> &ComparisonImplementation {
        &self.implementation
    }

    #[must_use]
    pub const fn reverse(&self) -> bool {
        self.reverse
    }

    #[must_use]
    pub const fn negate(&self) -> bool {
        self.negate
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LogicalOperation {
    And,
    Or,
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
    next: StaticSelection,
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
    pub const fn next(&self) -> &StaticSelection {
        &self.next
    }

    #[must_use]
    pub const fn item(&self) -> TypeId {
        self.item
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IterationAcquisition {
    Direct,
    Expansion(StaticSelection),
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
        exact_size: StaticSelection,
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
        formatter: StaticSelection,
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
    pub(crate) const fn new(kind: LoopKind, body: BodyNodeId) -> Self {
        Self { kind, body }
    }

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
        scope: BodyScopeId,
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
    /// Retains a checked source subtree that has no incoming executable edge.
    Unreachable(BodyNodeId),
    Return(Option<BodyNodeId>),
    Break(LoopId),
    Continue(LoopId),
    Drop(PlaceId),
    If {
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
    },
    Logical {
        operation: LogicalOperation,
        left: BodyNodeId,
        right: BodyNodeId,
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
