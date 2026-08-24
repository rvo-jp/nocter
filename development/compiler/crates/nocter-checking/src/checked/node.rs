pub use nocter_model::ConstantValue;
use nocter_model::{
    BodyNodeId, BodyScopeId, BorrowCapability, CallableCapability, CaptureId, ClosureId, FieldId,
    LocalBindingId, LoopId, NominalTypeId, ParameterId, PlaceId, TypeId, VariantId,
};

use super::{ArgumentPackSegment, CheckedArgumentPack};

use crate::expected::OutcomeLayer;

use super::{DropSelection, StaticSelection};

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

    pub(super) fn replace_operation(&mut self, operation: CheckedOperation) {
        self.operation = operation;
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
    OpaqueWitness(super::CheckedOpaqueWitness),
    Closure(CheckedClosure),
    ArgumentPackLength(ParameterId),
    IteratorAcquisition(CheckedIteratorAcquisition),
    Sequence(CheckedSequence),
    StringLiteral {
        constructor: StaticSelection,
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
pub enum CallTarget {
    Static(StaticSelection),
    ClosureValue {
        value: BodyNodeId,
        closure: ClosureId,
        capability: CallableCapability,
    },
    CallableValue {
        value: BodyNodeId,
        capability: CallableCapability,
        dispatch: StaticSelection,
    },
}

/// How a source receiver is prepared for one selected method invocation.
///
/// The receiver value and this preparation form a closed lowering contract. Lowering never has
/// to recover whether an owned expression was a place or a temporary, nor whether an existing
/// readwrite borrow was preserved or weakened.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReceiverPreparation {
    Owned,
    BorrowPlace(BorrowCapability),
    BorrowTemporary(BorrowCapability),
    PreserveBorrow(BorrowCapability),
    WeakenReadwriteBorrow,
}

/// How a selected borrow coercion result supplies the method receiver.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CoercedReceiverPreparation {
    PreserveReadonly,
    PreserveReadwrite,
    WeakenReadwrite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedReceiverCoercion {
    selection: StaticSelection,
    result_preparation: CoercedReceiverPreparation,
}

impl CheckedReceiverCoercion {
    pub(crate) const fn new(
        selection: StaticSelection,
        result_preparation: CoercedReceiverPreparation,
    ) -> Self {
        Self {
            selection,
            result_preparation,
        }
    }

    #[must_use]
    pub const fn selection(&self) -> &StaticSelection {
        &self.selection
    }

    #[must_use]
    pub const fn result_preparation(&self) -> CoercedReceiverPreparation {
        self.result_preparation
    }
}

/// One completely selected receiver, including an optional one-step borrow coercion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedReceiver {
    value: BodyNodeId,
    preparation: ReceiverPreparation,
    coercion: Option<CheckedReceiverCoercion>,
}

impl CheckedReceiver {
    pub(crate) const fn new(
        value: BodyNodeId,
        preparation: ReceiverPreparation,
        coercion: Option<CheckedReceiverCoercion>,
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
    pub const fn preparation(&self) -> ReceiverPreparation {
        self.preparation
    }

    #[must_use]
    pub const fn coercion(&self) -> Option<&CheckedReceiverCoercion> {
        self.coercion.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCall {
    target: CallTarget,
    receiver: Option<CheckedReceiver>,
    arguments: Box<[BodyNodeId]>,
    pack: Option<CheckedArgumentPack>,
}

impl CheckedCall {
    pub(crate) fn new(
        target: CallTarget,
        receiver: Option<CheckedReceiver>,
        arguments: impl Into<Box<[BodyNodeId]>>,
        pack: Option<CheckedArgumentPack>,
    ) -> Self {
        Self {
            target,
            receiver,
            arguments: arguments.into(),
            pack,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &CallTarget {
        &self.target
    }

    #[must_use]
    pub const fn receiver(&self) -> Option<&CheckedReceiver> {
        self.receiver.as_ref()
    }

    #[must_use]
    pub const fn arguments(&self) -> &[BodyNodeId] {
        &self.arguments
    }

    #[must_use]
    pub const fn pack(&self) -> Option<&CheckedArgumentPack> {
        self.pack.as_ref()
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

/// How one source operand becomes the readonly receiver of a compiler-selected operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadonlyOperandPreparation {
    BorrowPlace,
    BorrowTemporary,
    UseReadonlyBorrow,
    WeakenReadwriteBorrow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedReadonlyOperand {
    value: BodyNodeId,
    preparation: ReadonlyOperandPreparation,
    coercion: Option<StaticSelection>,
}

impl CheckedReadonlyOperand {
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
    left: CheckedReadonlyOperand,
    right: CheckedReadonlyOperand,
    implementation: ComparisonImplementation,
    reverse: bool,
    negate: bool,
}

impl CheckedComparison {
    pub(crate) const fn new(
        operation: ComparisonOperation,
        left: CheckedReadonlyOperand,
        right: CheckedReadonlyOperand,
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
    pub const fn left(&self) -> &CheckedReadonlyOperand {
        &self.left
    }

    #[must_use]
    pub const fn right(&self) -> &CheckedReadonlyOperand {
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
        outer: Box<[OutcomeLayer]>,
    },
    Force {
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
    closure: ClosureId,
    captures: Box<[CheckedClosureCapture]>,
}

impl CheckedClosure {
    pub(crate) fn new(
        closure: ClosureId,
        captures: impl Into<Box<[CheckedClosureCapture]>>,
    ) -> Self {
        Self {
            closure,
            captures: captures.into(),
        }
    }

    #[must_use]
    pub const fn closure(&self) -> ClosureId {
        self.closure
    }

    #[must_use]
    pub const fn captures(&self) -> &[CheckedClosureCapture] {
        &self.captures
    }
}

/// One source-order closure-environment initialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedClosureCapture {
    binding: CaptureId,
    initializer: BodyNodeId,
}

impl CheckedClosureCapture {
    pub(crate) const fn new(binding: CaptureId, initializer: BodyNodeId) -> Self {
        Self {
            binding,
            initializer,
        }
    }

    #[must_use]
    pub const fn binding(self) -> CaptureId {
        self.binding
    }

    #[must_use]
    pub const fn initializer(self) -> BodyNodeId {
        self.initializer
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AllocationSelection {
    CurrentRegion,
    Explicit(BodyNodeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedIteratorAcquisition {
    source: CheckedReceiver,
    acquisition: IterationAcquisition,
}

impl CheckedIteratorAcquisition {
    pub(crate) const fn new(source: CheckedReceiver, acquisition: IterationAcquisition) -> Self {
        Self {
            source,
            acquisition,
        }
    }

    #[must_use]
    pub const fn source(&self) -> &CheckedReceiver {
        &self.source
    }

    #[must_use]
    pub const fn acquisition(&self) -> &IterationAcquisition {
        &self.acquisition
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedIteration {
    iterator: BodyNodeId,
    next: StaticSelection,
    item: TypeId,
}

impl TypedIteration {
    pub(crate) const fn new(iterator: BodyNodeId, next: StaticSelection, item: TypeId) -> Self {
        Self {
            iterator,
            next,
            item,
        }
    }

    #[must_use]
    pub const fn iterator(&self) -> BodyNodeId {
        self.iterator
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedSequence {
    constructor: StaticSelection,
    pack: CheckedArgumentPack,
    allocation: AllocationSelection,
}

impl CheckedSequence {
    pub(crate) fn new(
        constructor: StaticSelection,
        segments: impl Into<Box<[ArgumentPackSegment]>>,
        allocation: AllocationSelection,
    ) -> Self {
        Self {
            constructor,
            pack: CheckedArgumentPack::new(segments),
            allocation,
        }
    }

    #[must_use]
    pub const fn constructor(&self) -> &StaticSelection {
        &self.constructor
    }

    #[must_use]
    pub const fn pack(&self) -> &CheckedArgumentPack {
        &self.pack
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
        operand: CheckedReadonlyOperand,
        formatter: StaticSelection,
    },
    Diverging(BodyNodeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInterpolation {
    constructor: StaticSelection,
    text_appender: StaticSelection,
    parts: Box<[InterpolationPart]>,
    output: TypeId,
    allocation: AllocationSelection,
}

impl CheckedInterpolation {
    pub(crate) fn new(
        constructor: StaticSelection,
        text_appender: StaticSelection,
        parts: impl Into<Box<[InterpolationPart]>>,
        output: TypeId,
        allocation: AllocationSelection,
    ) -> Self {
        Self {
            constructor,
            text_appender,
            parts: parts.into(),
            output,
            allocation,
        }
    }

    #[must_use]
    pub const fn constructor(&self) -> &StaticSelection {
        &self.constructor
    }

    #[must_use]
    pub const fn text_appender(&self) -> &StaticSelection {
        &self.text_appender
    }

    #[must_use]
    pub const fn parts(&self) -> &[InterpolationPart] {
        &self.parts
    }

    #[must_use]
    pub const fn output(&self) -> TypeId {
        self.output
    }

    #[must_use]
    pub const fn allocation(&self) -> AllocationSelection {
        self.allocation
    }
}

/// How one checked enum value supplies tag inspection and payload bindings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PatternSubjectPreparation {
    OwnedTemporary,
    RetainedPlace,
    ConsumedPlace,
    Borrowed(BorrowCapability),
}

/// The already-proved storage operation that initializes one named pattern payload.
///
/// This decision belongs to body checking: later lowering must not repeat copyability proof or
/// infer ownership from the presence of a type-owned drop body.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PatternBindingMode {
    Copy,
    Move,
    Borrow(BorrowCapability),
}

/// The exact owned storage left behind after one pattern arm initializes its bindings.
///
/// Ownership analysis decides only when this remainder is live. Its structural shape is frozen by
/// body checking so cleanup planning and MIR lowering never reconstruct payload transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternRemainder {
    NoCleanup,
    Complete,
    Residual(Box<[ParameterId]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedPatternSubject {
    value: BodyNodeId,
    nominal: NominalTypeId,
    preparation: PatternSubjectPreparation,
}

impl CheckedPatternSubject {
    pub(crate) const fn new(
        value: BodyNodeId,
        nominal: NominalTypeId,
        preparation: PatternSubjectPreparation,
    ) -> Self {
        Self {
            value,
            nominal,
            preparation,
        }
    }

    #[must_use]
    pub const fn value(self) -> BodyNodeId {
        self.value
    }

    #[must_use]
    pub const fn nominal(self) -> NominalTypeId {
        self.nominal
    }

    #[must_use]
    pub const fn preparation(self) -> PatternSubjectPreparation {
        self.preparation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedPatternSlot {
    parameter: ParameterId,
    binding: Option<PatternBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PatternBinding {
    local: LocalBindingId,
    mode: PatternBindingMode,
}

impl CheckedPatternSlot {
    pub(crate) const fn new(
        parameter: ParameterId,
        binding: Option<(LocalBindingId, PatternBindingMode)>,
    ) -> Self {
        Self {
            parameter,
            binding: match binding {
                Some((local, mode)) => Some(PatternBinding { local, mode }),
                None => None,
            },
        }
    }

    #[must_use]
    pub const fn parameter(self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn binding(self) -> Option<LocalBindingId> {
        match self.binding {
            Some(binding) => Some(binding.local),
            None => None,
        }
    }

    #[must_use]
    pub const fn binding_mode(self) -> Option<PatternBindingMode> {
        match self.binding {
            Some(binding) => Some(binding.mode),
            None => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPattern {
    variant: VariantId,
    slots: Box<[CheckedPatternSlot]>,
    before_transfer_drop: Option<DropSelection>,
    remainder: PatternRemainder,
}

impl CheckedPattern {
    pub(crate) fn new(
        variant: VariantId,
        slots: impl Into<Box<[CheckedPatternSlot]>>,
        before_transfer_drop: Option<DropSelection>,
        remainder: PatternRemainder,
    ) -> Self {
        Self {
            variant,
            slots: slots.into(),
            before_transfer_drop,
            remainder,
        }
    }

    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn slots(&self) -> &[CheckedPatternSlot] {
        &self.slots
    }

    /// The type-owned drop body that observes complete `Self` before move-only payload transfer.
    #[must_use]
    pub const fn before_transfer_drop(&self) -> Option<&DropSelection> {
        self.before_transfer_drop.as_ref()
    }

    #[must_use]
    pub const fn remainder(&self) -> &PatternRemainder {
        &self.remainder
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPatternArm {
    pattern: CheckedPattern,
    body: BodyNodeId,
}

impl CheckedPatternArm {
    pub(crate) const fn new(pattern: CheckedPattern, body: BodyNodeId) -> Self {
        Self { pattern, body }
    }

    #[must_use]
    pub const fn pattern(&self) -> &CheckedPattern {
        &self.pattern
    }

    #[must_use]
    pub const fn body(&self) -> BodyNodeId {
        self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedPatternFallback {
    body: BodyNodeId,
    reachable: bool,
}

impl CheckedPatternFallback {
    pub(crate) const fn new(body: BodyNodeId, reachable: bool) -> Self {
        Self { body, reachable }
    }

    #[must_use]
    pub const fn body(self) -> BodyNodeId {
        self.body
    }

    #[must_use]
    pub const fn reachable(self) -> bool {
        self.reachable
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
    ArgumentPack {
        binding: LocalBindingId,
        parameter: ParameterId,
        item: TypeId,
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
    Pattern {
        subject: CheckedPatternSubject,
        arms: Box<[CheckedPatternArm]>,
        fallback: Option<CheckedPatternFallback>,
        /// A source-level non-match path exists without an explicit body (`if is` without else).
        unmatched: bool,
    },
    Loop(LoopId),
    Region {
        binding: LocalBindingId,
        allocator: BodyNodeId,
        body: BodyNodeId,
    },
}
