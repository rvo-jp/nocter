use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use nocter_checking::{
    ConcreteDestructionError, ConcreteDestructionPlan, ConcreteDispatchError, GenericArguments,
    ResolvedPrimitiveDispatch, StaticSelection, is_concrete_type,
};
use nocter_model::{
    Arena, BodyId, BodyNodeId, BorrowCapability, ClosureId, ExecutableItemId, PackageTargetId,
    TestId, TypeId, TypeStore,
};
use nocter_runtime_contract::{
    RuntimeEnvironment, RuntimeEnvironmentError, RuntimeTypeTableBuildError,
};

use crate::{
    BodyDependencyError, CallableInstanceKey, CallableInstanceKeyError, CheckedDestruction,
    ClosureInstanceKey, ClosureInstanceKeyError, DropInstanceKey, DropInstanceKeyError,
    EntrySelectionError, PrimitiveRole, ProcessResultContract, RuntimeTypeRepresentationTable,
    TargetProgram, TestSelectionError,
};
use semantic_environment::ExecutableSemanticEnvironment;

mod argument_pack;
mod build;
mod callable_invocation;
mod closure_layout;
mod pack_literal;
mod primitive_dependency;
mod semantic_environment;
mod signature;
mod type_representation;

/// Borrowed proof that a target and type store come from one executable-specialization authority.
///
/// Only the executable closure builder can create this value. Instance-key constructors consume it
/// instead of accepting independently pairable target and type-store references.
#[derive(Clone, Copy)]
pub(crate) struct ExecutableSpecialization<'a> {
    target: &'a TargetProgram,
    types: &'a TypeStore,
}

impl<'a> ExecutableSpecialization<'a> {
    const fn new(target: &'a TargetProgram, types: &'a TypeStore) -> Self {
        Self { target, types }
    }

    pub(crate) const fn target(self) -> &'a TargetProgram {
        self.target
    }

    pub(crate) const fn types(self) -> &'a TypeStore {
        self.types
    }
}

pub(crate) use argument_pack::ExecutablePackIteration;
pub use argument_pack::{ExecutableArgumentPackPlan, ExecutablePackSegment, ExecutablePackSpread};
pub use callable_invocation::ExecutableCallableInvocation;
pub use closure_layout::{ExecutableClosureCapture, ExecutableClosureLayout};
pub use pack_literal::ExecutablePackLiteralPlan;
pub use primitive_dependency::ExecutablePrimitiveDependency;
pub use signature::{
    ExecutableInput, ExecutableInputSource, ExecutablePackInput, ExecutableSignature,
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
    pack_literals: Box<[ExecutablePackLiteralPlan]>,
    argument_packs: Box<[ExecutableArgumentPackPlan]>,
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
            .binary_search_by(|edge| edge.source.cmp(source))
            .ok()
            .map(|index| &self.dispatches[index].plan)
    }

    #[must_use]
    pub const fn closures(&self) -> &[ExecutableClosureEdge] {
        &self.closures
    }

    #[must_use]
    pub fn closure_item(&self, closure: ClosureId) -> Option<ExecutableItemId> {
        self.closures
            .binary_search_by_key(&closure, |edge| edge.closure)
            .ok()
            .map(|index| self.closures[index].item)
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
            .binary_search_by(|edge| edge.selection.cmp(selection))
            .ok()
            .map(|index| self.drops[index].item)
    }

    #[must_use]
    pub const fn types(&self) -> &[ExecutableTypeEdge] {
        &self.types
    }

    #[must_use]
    pub fn concrete_type(&self, source: TypeId) -> Option<TypeId> {
        self.types
            .binary_search_by_key(&source, |edge| edge.source)
            .ok()
            .map(|index| self.types[index].concrete)
    }

    #[must_use]
    pub fn prepared_borrow(&self, source: TypeId, capability: BorrowCapability) -> Option<TypeId> {
        self.prepared_borrows
            .binary_search_by_key(&(source, capability), |edge| (edge.source, edge.capability))
            .ok()
            .map(|index| self.prepared_borrows[index].concrete)
    }

    #[must_use]
    pub fn complete_destruction_for_source(
        &self,
        source: TypeId,
    ) -> Option<&ConcreteDestructionPlan> {
        self.destructions
            .binary_search_by(|edge| edge.source.cmp(&CheckedDestruction::Complete(source)))
            .ok()
            .map(|index| &self.destructions[index].plan)
    }

    #[must_use]
    pub fn cleanup_destruction(
        &self,
        target: &nocter_checking::CleanupTarget,
    ) -> Option<&ConcreteDestructionPlan> {
        let source = CheckedDestruction::for_cleanup(target)?;
        self.destructions
            .binary_search_by(|edge| edge.source.cmp(&source))
            .ok()
            .map(|index| &self.destructions[index].plan)
    }

    #[must_use]
    pub const fn pack_literals(&self) -> &[ExecutablePackLiteralPlan] {
        &self.pack_literals
    }

    #[must_use]
    pub fn pack_literal(&self, source: BodyNodeId) -> Option<&ExecutablePackLiteralPlan> {
        self.pack_literals
            .binary_search_by_key(&source, ExecutablePackLiteralPlan::source)
            .ok()
            .map(|index| &self.pack_literals[index])
    }

    #[must_use]
    pub fn argument_pack(&self, source: BodyNodeId) -> Option<&ExecutableArgumentPackPlan> {
        self.argument_packs
            .binary_search_by_key(&source, ExecutableArgumentPackPlan::source)
            .ok()
            .map(|index| &self.argument_packs[index])
    }
}

/// One dense executable item and the body facts frozen for MIR lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableItem {
    key: ExecutableItemKey,
    signature: ExecutableSignature,
    accepts_allocation_override: bool,
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
    pub const fn accepts_allocation_override(&self) -> bool {
        self.accepts_allocation_override
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableTestCase {
    declaration: TestId,
    name: Box<str>,
    item: ExecutableItemId,
}

impl ExecutableTestCase {
    #[must_use]
    pub const fn declaration(&self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn item(&self) -> ExecutableItemId {
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
    semantic_environment: ExecutableSemanticEnvironment,
    types: TypeStore,
    checked_bodies: BTreeMap<BodyId, nocter_checking::CheckedBody>,
    items: Arena<ExecutableItemId, ExecutableItem>,
    runtime: RuntimeEnvironment,
    root: ExecutableRoot,
}

impl ExecutableProgram {
    /// Selects and closes one process executable.
    ///
    /// # Errors
    ///
    /// Returns the first entry-selection or executable-closure invariant failure.
    pub fn for_executable(
        target: impl Into<Arc<TargetProgram>>,
        selected: PackageTargetId,
    ) -> Result<Self, ExecutableProgramError> {
        build::build_executable(&target.into(), selected)
    }

    /// Selects and closes one compiler-owned test runner.
    ///
    /// # Errors
    ///
    /// Returns the first test-selection or executable-closure invariant failure.
    pub fn for_tests(
        target: impl Into<Arc<TargetProgram>>,
        selected: PackageTargetId,
    ) -> Result<Self, ExecutableProgramError> {
        build::build_tests(&target.into(), selected)
    }

    /// Closes an already selected semantic test set.
    ///
    /// # Errors
    ///
    /// Returns the first executable-closure invariant failure.
    pub fn for_selected_tests(
        target: impl Into<Arc<TargetProgram>>,
        selected: &crate::SelectedTestTarget,
    ) -> Result<Self, ExecutableProgramError> {
        build::build_selected_tests(&target.into(), selected)
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
    pub fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout> {
        self.items.get(item)?.closure_layout()
    }

    #[must_use]
    pub const fn type_representations(&self) -> &RuntimeTypeRepresentationTable {
        self.runtime.type_representations()
    }

    #[must_use]
    pub const fn root(&self) -> &ExecutableRoot {
        &self.root
    }

    /// Consumes the executable closure and retains only the facts admitted past the MIR boundary.
    #[must_use]
    pub fn into_runtime_environment(self) -> RuntimeEnvironment {
        self.runtime
    }
}

fn build_runtime_environment(
    types: &TypeStore,
    representations: RuntimeTypeRepresentationTable,
    abi: nocter_runtime_contract::RuntimeAbiIdentity,
) -> Result<RuntimeEnvironment, ExecutableProgramError> {
    let runtime_types = runtime_type_table(types)?;
    RuntimeEnvironment::new(runtime_types, representations, abi)
        .map_err(ExecutableProgramError::RuntimeEnvironment)
}

fn runtime_type_table(
    types: &TypeStore,
) -> Result<nocter_runtime_contract::RuntimeTypeTable, ExecutableProgramError> {
    use nocter_model::{BuiltinType, TypeKind};
    use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTableBuilder};

    let mut table = RuntimeTypeTableBuilder::new();
    for (ty, kind) in types.iter() {
        if !is_concrete_type(types, ty)
            .map_err(|_| ExecutableProgramError::InvalidTypeRepresentation(ty))?
        {
            continue;
        }
        let runtime = match kind {
            TypeKind::Builtin(builtin) => RuntimeType::Primitive(match builtin {
                BuiltinType::Bool => RuntimePrimitive::Bool,
                BuiltinType::I8 => RuntimePrimitive::Signed(8),
                BuiltinType::I16 => RuntimePrimitive::Signed(16),
                BuiltinType::I32 => RuntimePrimitive::Signed(32),
                BuiltinType::I64 => RuntimePrimitive::Signed(64),
                BuiltinType::U8 => RuntimePrimitive::Unsigned(8),
                BuiltinType::U16 => RuntimePrimitive::Unsigned(16),
                BuiltinType::U32 => RuntimePrimitive::Unsigned(32),
                BuiltinType::U64 => RuntimePrimitive::Unsigned(64),
                BuiltinType::Isize => RuntimePrimitive::Isize,
                BuiltinType::Usize => RuntimePrimitive::Usize,
                BuiltinType::Error => RuntimePrimitive::Error,
                BuiltinType::Str => RuntimePrimitive::Text,
                BuiltinType::Void => RuntimePrimitive::Void,
                BuiltinType::Never => RuntimePrimitive::Never,
            }),
            TypeKind::Pointer(pointee) => RuntimeType::Pointer(*pointee),
            TypeKind::Borrow {
                capability,
                referent,
            } => RuntimeType::Borrow {
                capability: *capability,
                referent: *referent,
            },
            TypeKind::Slice(element) => RuntimeType::Slice(*element),
            TypeKind::FixedArray { element, length } => RuntimeType::FixedArray {
                element: *element,
                length: *length,
            },
            TypeKind::PackEntry { key, value } => RuntimeType::PackEntry {
                key: *key,
                value: *value,
            },
            TypeKind::Nominal { .. } => RuntimeType::Aggregate,
            TypeKind::Closure { .. } => RuntimeType::Closure,
            TypeKind::Callable(_) => RuntimeType::Callable,
            TypeKind::Optional(payload) => RuntimeType::Optional(*payload),
            TypeKind::Fallible(payload) => RuntimeType::Fallible(*payload),
            TypeKind::Opaque { .. } => RuntimeType::Opaque,
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => {
                return Err(ExecutableProgramError::InvalidTypeRepresentation(ty));
            }
        };
        table
            .insert(ty, runtime)
            .map_err(ExecutableProgramError::RuntimeTypes)?;
    }
    table.finish().map_err(ExecutableProgramError::RuntimeTypes)
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
    MissingTestName(TestId),
    MissingParameter(nocter_model::ParameterId),
    MissingRoot(BodyNodeId),
    InvalidArgumentPackSignature(nocter_model::CallableId),
    InvalidArgumentPackPlan(BodyNodeId),
    InvalidPackLiteralPlan(BodyNodeId),
    InvalidCallableInvocation(TypeId),
    InvalidPrimitiveDependency(PrimitiveRole),
    RuntimeTypes(RuntimeTypeTableBuildError),
    RuntimeEnvironment(RuntimeEnvironmentError),
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
            Self::RuntimeTypes(error) => Some(error),
            Self::RuntimeEnvironment(error) => Some(error),
            Self::DuplicateGeneric(_)
            | Self::UnknownBody(_)
            | Self::UnknownItem(_)
            | Self::BodylessCallable(_)
            | Self::MissingTestName(_)
            | Self::MissingParameter(_)
            | Self::MissingRoot(_)
            | Self::InvalidArgumentPackSignature(_)
            | Self::InvalidArgumentPackPlan(_)
            | Self::InvalidPackLiteralPlan(_)
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
