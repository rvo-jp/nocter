use std::collections::BTreeMap;
use std::fmt;

use nocter_checking::{
    ConcreteDestructionError, ConcreteDestructionPlan, ConcreteDispatchError, GenericArguments,
    ResolvedPrimitiveDispatch, StaticSelection,
};
use nocter_model::{
    Arena, BodyId, BodyNodeId, BorrowCapability, ClosureId, ExecutableItemId, PackageTargetId,
    Symbol, TestId, TypeId, TypeStore,
};

use crate::{
    BodyDependencyError, CallableInstanceKey, CallableInstanceKeyError, CheckedDestruction,
    ClosureInstanceKey, ClosureInstanceKeyError, DropInstanceKey, DropInstanceKeyError,
    EntrySelectionError, PrimitiveRole, ProcessResultContract, TargetProgram, TestSelectionError,
};

mod build;
mod callable_invocation;
mod closure_layout;
mod primitive_dependency;
mod sequence_pack;
mod signature;
mod type_representation;

pub use callable_invocation::ExecutableCallableInvocation;
pub use closure_layout::{ExecutableClosureCapture, ExecutableClosureLayout};
pub use primitive_dependency::ExecutablePrimitiveDependency;
pub(crate) use sequence_pack::ExecutableSequenceIteration;
pub use sequence_pack::{
    ExecutableSequencePlan, ExecutableSequenceSegment, ExecutableSequenceSpread,
};
pub use signature::{
    ExecutableInput, ExecutableInputSource, ExecutablePackInput, ExecutableSignature,
};
pub use type_representation::{
    ExecutableFieldRepresentation, ExecutablePayloadRepresentation, ExecutableTypeRepresentation,
    ExecutableTypeRepresentationTable, ExecutableVariantRepresentation,
};

/// One canonical monomorphized source-body identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutableItemKey {
    Callable(CallableInstanceKey),
    Closure(ClosureInstanceKey),
    Drop(DropInstanceKey),
    Test(TestId),
}

/// One concrete standard primitive invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutablePrimitiveCall {
    role: PrimitiveRole,
    generic_arguments: GenericArguments,
    signature: ExecutableSignature,
    dependency: ExecutablePrimitiveDependency,
}

impl ExecutablePrimitiveCall {
    #[must_use]
    pub const fn role(&self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }

    #[must_use]
    pub const fn signature(&self) -> &ExecutableSignature {
        &self.signature
    }

    #[must_use]
    pub const fn dependency(&self) -> &ExecutablePrimitiveDependency {
        &self.dependency
    }
}

/// One executable dispatch step after all static evidence has been consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableDispatchStep {
    Direct(ExecutableItemId),
    StandardPrimitive(ExecutablePrimitiveCall),
    StructuralPrimitive(ResolvedPrimitiveDispatch),
    CallableValue(ExecutableCallableInvocation),
}

/// One fully specialized opaque receiver representation change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableOpaqueReceiver {
    definition: nocter_model::OpaqueTypeId,
    opaque: TypeId,
    witness: TypeId,
    source: TypeId,
    target: TypeId,
}

impl ExecutableOpaqueReceiver {
    #[must_use]
    pub const fn definition(self) -> nocter_model::OpaqueTypeId {
        self.definition
    }

    #[must_use]
    pub const fn opaque(self) -> TypeId {
        self.opaque
    }

    #[must_use]
    pub const fn witness(self) -> TypeId {
        self.witness
    }

    #[must_use]
    pub const fn source(self) -> TypeId {
        self.source
    }

    #[must_use]
    pub const fn target(self) -> TypeId {
        self.target
    }
}

/// One frozen dispatch plan with every composite operand lane kept explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableDispatchPlan {
    Invocation(ExecutableDispatchStep),
    OpaqueInvocation {
        receiver: ExecutableOpaqueReceiver,
        operation: ExecutableDispatchStep,
    },
    Comparison {
        left_coercion: Option<ExecutableDispatchStep>,
        right_coercion: Option<ExecutableDispatchStep>,
        operation: ExecutableDispatchStep,
    },
    Index {
        receiver_coercion: Option<ExecutableDispatchStep>,
        operation: ExecutableDispatchStep,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableDispatchEdge {
    source: StaticSelection,
    plan: ExecutableDispatchPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableDestructionEdge {
    source: CheckedDestruction,
    plan: ConcreteDestructionPlan,
}

/// One nested closure edge resolved to its executable item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableClosureEdge {
    closure: ClosureId,
    item: ExecutableItemId,
}

impl ExecutableClosureEdge {
    #[must_use]
    pub const fn closure(self) -> ClosureId {
        self.closure
    }

    #[must_use]
    pub const fn item(self) -> ExecutableItemId {
        self.item
    }
}

/// One exact checked drop selection resolved to its executable item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableDropEdge {
    selection: nocter_checking::DropSelection,
    item: ExecutableItemId,
}

impl ExecutableDropEdge {
    #[must_use]
    pub const fn selection(&self) -> &nocter_checking::DropSelection {
        &self.selection
    }

    #[must_use]
    pub const fn item(&self) -> ExecutableItemId {
        self.item
    }
}

/// One checked body type and its canonical concrete specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableTypeEdge {
    source: TypeId,
    concrete: TypeId,
}

/// One concrete borrow type introduced by checked operand preparation rather than a source node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableBorrowEdge {
    source: TypeId,
    capability: BorrowCapability,
    concrete: TypeId,
}

impl ExecutableBorrowEdge {
    #[must_use]
    pub const fn source(self) -> TypeId {
        self.source
    }

    #[must_use]
    pub const fn capability(self) -> BorrowCapability {
        self.capability
    }

    #[must_use]
    pub const fn concrete(self) -> TypeId {
        self.concrete
    }
}

impl ExecutableTypeEdge {
    #[must_use]
    pub const fn source(self) -> TypeId {
        self.source
    }

    #[must_use]
    pub const fn concrete(self) -> TypeId {
        self.concrete
    }
}

/// Frozen lowering input for one concrete checked-body root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableBody {
    body: BodyId,
    root: BodyNodeId,
    nodes: Box<[BodyNodeId]>,
    dispatches: Box<[ExecutableDispatchEdge]>,
    closures: Box<[ExecutableClosureEdge]>,
    drops: Box<[ExecutableDropEdge]>,
    types: Box<[ExecutableTypeEdge]>,
    prepared_borrows: Box<[ExecutableBorrowEdge]>,
    destructions: Box<[ExecutableDestructionEdge]>,
    sequences: Box<[ExecutableSequencePlan]>,
}

impl ExecutableBody {
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn root(&self) -> BodyNodeId {
        self.root
    }

    #[must_use]
    pub const fn nodes(&self) -> &[BodyNodeId] {
        &self.nodes
    }

    #[must_use]
    pub fn dispatch(&self, source: &StaticSelection) -> Option<&ExecutableDispatchPlan> {
        self.dispatches
            .iter()
            .find(|edge| edge.source == *source)
            .map(|edge| &edge.plan)
    }

    #[must_use]
    pub const fn closures(&self) -> &[ExecutableClosureEdge] {
        &self.closures
    }

    #[must_use]
    pub fn closure_item(&self, closure: ClosureId) -> Option<ExecutableItemId> {
        self.closures
            .iter()
            .find(|edge| edge.closure == closure)
            .map(|edge| edge.item)
    }

    #[must_use]
    pub const fn drops(&self) -> &[ExecutableDropEdge] {
        &self.drops
    }

    #[must_use]
    pub fn drop_item(
        &self,
        selection: &nocter_checking::DropSelection,
    ) -> Option<ExecutableItemId> {
        self.drops
            .iter()
            .find(|edge| edge.selection == *selection)
            .map(|edge| edge.item)
    }

    #[must_use]
    pub const fn types(&self) -> &[ExecutableTypeEdge] {
        &self.types
    }

    #[must_use]
    pub fn concrete_type(&self, source: TypeId) -> Option<TypeId> {
        self.types
            .iter()
            .find(|edge| edge.source == source)
            .map(|edge| edge.concrete)
    }

    #[must_use]
    pub fn prepared_borrow(&self, source: TypeId, capability: BorrowCapability) -> Option<TypeId> {
        self.prepared_borrows
            .iter()
            .find(|edge| edge.source == source && edge.capability == capability)
            .map(|edge| edge.concrete)
    }

    #[must_use]
    pub fn complete_destruction_for_source(
        &self,
        source: TypeId,
    ) -> Option<&ConcreteDestructionPlan> {
        self.destructions
            .iter()
            .find(|edge| edge.source == CheckedDestruction::Complete(source))
            .map(|edge| &edge.plan)
    }

    #[must_use]
    pub fn cleanup_destruction(
        &self,
        target: &nocter_checking::CleanupTarget,
    ) -> Option<&ConcreteDestructionPlan> {
        let source = CheckedDestruction::for_cleanup(target)?;
        self.destructions
            .iter()
            .find(|edge| edge.source == source)
            .map(|edge| &edge.plan)
    }

    #[must_use]
    pub const fn sequences(&self) -> &[ExecutableSequencePlan] {
        &self.sequences
    }

    #[must_use]
    pub fn sequence(&self, source: BodyNodeId) -> Option<&ExecutableSequencePlan> {
        self.sequences.iter().find(|plan| plan.source() == source)
    }
}

/// One dense executable item and the body facts frozen for MIR lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableItem {
    key: ExecutableItemKey,
    signature: ExecutableSignature,
    closure: Option<ExecutableClosureLayout>,
    body: ExecutableBody,
}

impl ExecutableItem {
    #[must_use]
    pub const fn key(&self) -> &ExecutableItemKey {
        &self.key
    }

    #[must_use]
    pub const fn signature(&self) -> &ExecutableSignature {
        &self.signature
    }

    #[must_use]
    pub const fn closure_layout(&self) -> Option<&ExecutableClosureLayout> {
        self.closure.as_ref()
    }

    #[must_use]
    pub const fn body(&self) -> &ExecutableBody {
        &self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableTestCase {
    declaration: TestId,
    name: Symbol,
    item: ExecutableItemId,
}

impl ExecutableTestCase {
    #[must_use]
    pub const fn declaration(self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn item(self) -> ExecutableItemId {
        self.item
    }
}

/// Compiler-owned root metadata without a synthetic source declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableRoot {
    Process {
        target: PackageTargetId,
        entry: ExecutableItemId,
        result: ProcessResultContract,
    },
    Tests {
        target: PackageTargetId,
        cases: Box<[ExecutableTestCase]>,
    },
}

/// The complete deterministic monomorphized closure for one selected package target.
#[derive(Debug)]
pub struct ExecutableProgram {
    target: TargetProgram,
    types: TypeStore,
    items: Arena<ExecutableItemId, ExecutableItem>,
    item_ids: BTreeMap<ExecutableItemKey, ExecutableItemId>,
    closure_layouts: BTreeMap<TypeId, ExecutableItemId>,
    type_representations: ExecutableTypeRepresentationTable,
    root: ExecutableRoot,
}

impl ExecutableProgram {
    /// Selects and closes one process executable.
    ///
    /// # Errors
    ///
    /// Returns the first entry-selection or executable-closure invariant failure.
    pub fn for_executable(
        target: TargetProgram,
        selected: PackageTargetId,
    ) -> Result<Self, ExecutableProgramError> {
        build::build_executable(target, selected)
    }

    /// Selects and closes one compiler-owned test runner.
    ///
    /// # Errors
    ///
    /// Returns the first test-selection or executable-closure invariant failure.
    pub fn for_tests(
        target: TargetProgram,
        selected: PackageTargetId,
    ) -> Result<Self, ExecutableProgramError> {
        build::build_tests(target, selected)
    }

    #[must_use]
    pub const fn target(&self) -> &TargetProgram {
        &self.target
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn items(&self) -> &Arena<ExecutableItemId, ExecutableItem> {
        &self.items
    }

    #[must_use]
    pub fn item_id(&self, key: &ExecutableItemKey) -> Option<ExecutableItemId> {
        self.item_ids.get(key).copied()
    }

    #[must_use]
    pub fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout> {
        self.items.get(item)?.closure_layout()
    }

    #[must_use]
    pub fn closure_capture_type(
        &self,
        closure_ty: TypeId,
        capture: nocter_model::CaptureId,
    ) -> Option<TypeId> {
        self.closure_layout_for_type(closure_ty)
            .and_then(|layout| layout.capture(capture))
            .map(ExecutableClosureCapture::ty)
    }

    #[must_use]
    pub fn closure_layout_for_type(&self, closure_ty: TypeId) -> Option<&ExecutableClosureLayout> {
        self.closure_layouts
            .get(&closure_ty)
            .and_then(|item| self.closure_layout(*item))
    }

    #[must_use]
    pub const fn type_representations(&self) -> &ExecutableTypeRepresentationTable {
        &self.type_representations
    }

    #[must_use]
    pub const fn root(&self) -> &ExecutableRoot {
        &self.root
    }
}

/// Failure to construct one closed executable program.
#[derive(Debug)]
pub enum ExecutableProgramError {
    EntrySelection(EntrySelectionError),
    TestSelection(TestSelectionError),
    CallableKey(CallableInstanceKeyError),
    ClosureKey(ClosureInstanceKeyError),
    DropKey(DropInstanceKeyError),
    Dependencies(BodyDependencyError),
    Dispatch(ConcreteDispatchError),
    Destruction(ConcreteDestructionError),
    DuplicateGeneric(nocter_model::GenericParameterId),
    UnknownBody(BodyId),
    UnknownItem(ExecutableItemKey),
    BodylessCallable(nocter_model::CallableId),
    MissingDeclarationSite(BodyId),
    MissingParameter(nocter_model::ParameterId),
    MissingRoot(BodyNodeId),
    InvalidClosureSignature(ClosureId),
    InvalidLiteralPackSignature(nocter_model::CallableId),
    InvalidSequencePlan(BodyNodeId),
    InvalidCallableInvocation(TypeId),
    InvalidPrimitiveDependency(PrimitiveRole),
    DuplicateClosureLayout(TypeId),
    InvalidTypeRepresentation(TypeId),
    MissingRepresentationField(nocter_model::FieldId),
    MissingRepresentationVariant(nocter_model::VariantId),
    MissingRepresentationParameter(nocter_model::ParameterId),
    MissingRepresentationWitness(nocter_model::OpaqueTypeId),
    DuplicateItem(ExecutableItemKey),
}

impl fmt::Display for ExecutableProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "executable-program construction failed: {self:?}"
        )
    }
}

impl std::error::Error for ExecutableProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EntrySelection(error) => Some(error),
            Self::TestSelection(error) => Some(error),
            Self::CallableKey(error) => Some(error),
            Self::ClosureKey(error) => Some(error),
            Self::DropKey(error) => Some(error),
            Self::Dependencies(error) => Some(error),
            Self::Dispatch(error) => Some(error),
            Self::Destruction(error) => Some(error),
            Self::DuplicateGeneric(_)
            | Self::UnknownBody(_)
            | Self::UnknownItem(_)
            | Self::BodylessCallable(_)
            | Self::MissingDeclarationSite(_)
            | Self::MissingParameter(_)
            | Self::MissingRoot(_)
            | Self::InvalidClosureSignature(_)
            | Self::InvalidLiteralPackSignature(_)
            | Self::InvalidSequencePlan(_)
            | Self::InvalidCallableInvocation(_)
            | Self::InvalidPrimitiveDependency(_)
            | Self::DuplicateClosureLayout(_)
            | Self::InvalidTypeRepresentation(_)
            | Self::MissingRepresentationField(_)
            | Self::MissingRepresentationVariant(_)
            | Self::MissingRepresentationParameter(_)
            | Self::MissingRepresentationWitness(_)
            | Self::DuplicateItem(_) => None,
        }
    }
}

impl From<EntrySelectionError> for ExecutableProgramError {
    fn from(error: EntrySelectionError) -> Self {
        Self::EntrySelection(error)
    }
}

impl From<TestSelectionError> for ExecutableProgramError {
    fn from(error: TestSelectionError) -> Self {
        Self::TestSelection(error)
    }
}

impl From<CallableInstanceKeyError> for ExecutableProgramError {
    fn from(error: CallableInstanceKeyError) -> Self {
        Self::CallableKey(error)
    }
}

impl From<ClosureInstanceKeyError> for ExecutableProgramError {
    fn from(error: ClosureInstanceKeyError) -> Self {
        Self::ClosureKey(error)
    }
}

impl From<DropInstanceKeyError> for ExecutableProgramError {
    fn from(error: DropInstanceKeyError) -> Self {
        Self::DropKey(error)
    }
}

impl From<BodyDependencyError> for ExecutableProgramError {
    fn from(error: BodyDependencyError) -> Self {
        Self::Dependencies(error)
    }
}

impl From<ConcreteDispatchError> for ExecutableProgramError {
    fn from(error: ConcreteDispatchError) -> Self {
        Self::Dispatch(error)
    }
}

impl From<ConcreteDestructionError> for ExecutableProgramError {
    fn from(error: ConcreteDestructionError) -> Self {
        Self::Destruction(error)
    }
}
