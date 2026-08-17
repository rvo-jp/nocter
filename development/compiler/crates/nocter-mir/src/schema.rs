use nocter_model::{
    Arena, ExecutableItemId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirPlaceId,
    MirValueId, PackageTargetId, Symbol, TestId, TypeId, VariantId,
};
use nocter_target_program::ProcessResultContract;

use crate::{MirLocal, MirOperation, MirPackInput, MirPlace};

/// One conditional-initialization bit associated with exact storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirDropFlag {
    place: MirPlaceId,
    initially_initialized: bool,
}

impl MirDropFlag {
    #[must_use]
    pub const fn new(place: MirPlaceId, initially_initialized: bool) -> Self {
        Self {
            place,
            initially_initialized,
        }
    }

    #[must_use]
    pub const fn place(self) -> MirPlaceId {
        self.place
    }

    #[must_use]
    pub const fn initially_initialized(self) -> bool {
        self.initially_initialized
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirValueDefinition {
    BlockParameter { block: MirBlockId, position: usize },
    Operation(MirOperationId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirValue {
    ty: TypeId,
    definition: MirValueDefinition,
}

impl MirValue {
    pub(crate) const fn new(ty: TypeId, definition: MirValueDefinition) -> Self {
        Self { ty, definition }
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn definition(self) -> MirValueDefinition {
        self.definition
    }
}

/// One CFG edge and the SSA values supplied to its destination block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBranchTarget {
    block: MirBlockId,
    arguments: Box<[MirValueId]>,
}

impl MirBranchTarget {
    #[must_use]
    pub fn new(block: MirBlockId, arguments: impl Into<Box<[MirValueId]>>) -> Self {
        Self {
            block,
            arguments: arguments.into(),
        }
    }

    #[must_use]
    pub const fn block(&self) -> MirBlockId {
        self.block
    }

    #[must_use]
    pub const fn arguments(&self) -> &[MirValueId] {
        &self.arguments
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirSwitchValue {
    Integer(i128),
    Variant(VariantId),
    OptionalPresent,
    OptionalAbsent,
    FallibleSuccess,
    FallibleFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSwitchCase {
    value: MirSwitchValue,
    target: MirBranchTarget,
}

impl MirSwitchCase {
    #[must_use]
    pub const fn new(value: MirSwitchValue, target: MirBranchTarget) -> Self {
        Self { value, target }
    }

    #[must_use]
    pub const fn value(&self) -> MirSwitchValue {
        self.value
    }

    #[must_use]
    pub const fn target(&self) -> &MirBranchTarget {
        &self.target
    }
}

/// The typed runtime subject inspected by a switch without moving aggregate storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSwitchSubject {
    Value(MirValueId),
    Place(MirPlaceId),
}

/// The sole control-transfer instruction of one basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminator {
    Goto(MirBranchTarget),
    Branch {
        condition: MirValueId,
        then_target: MirBranchTarget,
        else_target: MirBranchTarget,
    },
    BranchDropFlag {
        flag: MirDropFlagId,
        initialized: MirBranchTarget,
        uninitialized: MirBranchTarget,
    },
    Switch {
        subject: MirSwitchSubject,
        cases: Box<[MirSwitchCase]>,
        fallback: MirBranchTarget,
    },
    Return(Option<MirValueId>),
    /// Terminates one compiler-owned process root with status zero or an `i32`/`usize` value.
    Exit(Option<MirValueId>),
    Trap,
    Unreachable,
}

/// One basic block. Block parameters are ordinary SSA values defined at block entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    parameters: Box<[MirValueId]>,
    operations: Box<[MirOperationId]>,
    terminator: MirTerminator,
}

impl MirBlock {
    pub(crate) fn new(
        parameters: impl Into<Box<[MirValueId]>>,
        operations: impl Into<Box<[MirOperationId]>>,
        terminator: MirTerminator,
    ) -> Self {
        Self {
            parameters: parameters.into(),
            operations: operations.into(),
            terminator,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[MirValueId] {
        &self.parameters
    }

    #[must_use]
    pub const fn operations(&self) -> &[MirOperationId] {
        &self.operations
    }

    #[must_use]
    pub const fn terminator(&self) -> &MirTerminator {
        &self.terminator
    }
}

/// The storage, SSA, and control-flow domains shared by source functions and compiler roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBody {
    parameters: Box<[MirLocalId]>,
    pack: Option<MirPackInput>,
    locals: Arena<MirLocalId, MirLocal>,
    drop_flags: Arena<MirDropFlagId, MirDropFlag>,
    places: Arena<MirPlaceId, MirPlace>,
    values: Arena<MirValueId, MirValue>,
    operations: Arena<MirOperationId, MirOperation>,
    blocks: Arena<MirBlockId, MirBlock>,
    entry: MirBlockId,
}

pub(crate) struct MirBodyDomains {
    pub(crate) locals: Arena<MirLocalId, MirLocal>,
    pub(crate) drop_flags: Arena<MirDropFlagId, MirDropFlag>,
    pub(crate) places: Arena<MirPlaceId, MirPlace>,
    pub(crate) values: Arena<MirValueId, MirValue>,
    pub(crate) operations: Arena<MirOperationId, MirOperation>,
    pub(crate) blocks: Arena<MirBlockId, MirBlock>,
}

impl MirBody {
    pub(crate) fn new(
        parameters: impl Into<Box<[MirLocalId]>>,
        pack: Option<MirPackInput>,
        domains: MirBodyDomains,
        entry: MirBlockId,
    ) -> Self {
        Self {
            parameters: parameters.into(),
            pack,
            locals: domains.locals,
            drop_flags: domains.drop_flags,
            places: domains.places,
            values: domains.values,
            operations: domains.operations,
            blocks: domains.blocks,
            entry,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[MirLocalId] {
        &self.parameters
    }

    #[must_use]
    pub const fn pack(&self) -> Option<MirPackInput> {
        self.pack
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<MirLocalId, MirLocal> {
        &self.locals
    }

    #[must_use]
    pub const fn drop_flags(&self) -> &Arena<MirDropFlagId, MirDropFlag> {
        &self.drop_flags
    }

    #[must_use]
    pub const fn places(&self) -> &Arena<MirPlaceId, MirPlace> {
        &self.places
    }

    #[must_use]
    pub const fn values(&self) -> &Arena<MirValueId, MirValue> {
        &self.values
    }

    #[must_use]
    pub const fn operations(&self) -> &Arena<MirOperationId, MirOperation> {
        &self.operations
    }

    #[must_use]
    pub const fn blocks(&self) -> &Arena<MirBlockId, MirBlock> {
        &self.blocks
    }

    #[must_use]
    pub const fn entry(&self) -> MirBlockId {
        self.entry
    }
}

/// One complete monomorphized callable body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunction {
    item: ExecutableItemId,
    result: TypeId,
    body: MirBody,
}

impl MirFunction {
    pub(crate) const fn new(item: ExecutableItemId, result: TypeId, body: MirBody) -> Self {
        Self { item, result, body }
    }

    #[must_use]
    pub const fn item(&self) -> ExecutableItemId {
        self.item
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn body(&self) -> &MirBody {
        &self.body
    }

    #[must_use]
    pub const fn parameters(&self) -> &[MirLocalId] {
        self.body.parameters()
    }

    #[must_use]
    pub const fn pack(&self) -> Option<MirPackInput> {
        self.body.pack()
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<MirLocalId, MirLocal> {
        self.body.locals()
    }

    #[must_use]
    pub const fn drop_flags(&self) -> &Arena<MirDropFlagId, MirDropFlag> {
        self.body.drop_flags()
    }

    #[must_use]
    pub const fn places(&self) -> &Arena<MirPlaceId, MirPlace> {
        self.body.places()
    }

    #[must_use]
    pub const fn values(&self) -> &Arena<MirValueId, MirValue> {
        self.body.values()
    }

    #[must_use]
    pub const fn operations(&self) -> &Arena<MirOperationId, MirOperation> {
        self.body.operations()
    }

    #[must_use]
    pub const fn blocks(&self) -> &Arena<MirBlockId, MirBlock> {
        self.body.blocks()
    }

    #[must_use]
    pub const fn entry(&self) -> MirBlockId {
        self.body.entry()
    }
}

/// The allocation-free wrapper for one selected executable entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProcessRoot {
    target: PackageTargetId,
    entry: ExecutableItemId,
    result: ProcessResultContract,
    body: MirBody,
}

impl MirProcessRoot {
    pub(crate) const fn new(
        target: PackageTargetId,
        entry: ExecutableItemId,
        result: ProcessResultContract,
        body: MirBody,
    ) -> Self {
        Self {
            target,
            entry,
            result,
            body,
        }
    }

    #[must_use]
    pub const fn target(&self) -> PackageTargetId {
        self.target
    }

    #[must_use]
    pub const fn entry(&self) -> ExecutableItemId {
        self.entry
    }

    #[must_use]
    pub const fn result(&self) -> ProcessResultContract {
        self.result
    }

    #[must_use]
    pub const fn body(&self) -> &MirBody {
        &self.body
    }
}

/// One isolated compiler-owned test process in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTestRoot {
    declaration: TestId,
    name: Symbol,
    item: ExecutableItemId,
    body: MirBody,
}

impl MirTestRoot {
    pub(crate) const fn new(
        declaration: TestId,
        name: Symbol,
        item: ExecutableItemId,
        body: MirBody,
    ) -> Self {
        Self {
            declaration,
            name,
            item,
            body,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn item(&self) -> ExecutableItemId {
        self.item
    }

    #[must_use]
    pub const fn body(&self) -> &MirBody {
        &self.body
    }
}

/// Compiler-owned process identities, never represented as source declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirRoot {
    Process(MirProcessRoot),
    Tests {
        target: PackageTargetId,
        cases: Box<[MirTestRoot]>,
    },
}
