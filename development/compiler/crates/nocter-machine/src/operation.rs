use nocter_model::TypeId;

use crate::{
    MachineAddressId, MachineCall, MachineDataId, MachineDropFlagId, MachineFunctionId,
    MachineValueId,
};

/// A fully materialized constant. Text refers to the program's canonical static-data table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineConstant {
    Bool(bool),
    Character(u32),
    Float32(u32),
    Float64(u64),
    Integer(i128),
    Text(MachineDataId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineUnaryOperation {
    LogicalNot,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRightSigned,
    ShiftRightUnsigned,
    Equal,
    Less,
}

/// One initialized byte-range contribution to an aggregate value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAggregateWrite {
    Tag {
        offset: u64,
        value: u8,
    },
    Value {
        offset: u64,
        value: MachineValueId,
    },
    RepeatedValue {
        offset: u64,
        stride: u64,
        count: u64,
        value: MachineValueId,
    },
}

/// One aggregate assembled from exact layout-owned offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineAggregate {
    size: u64,
    alignment: u64,
    writes: Box<[MachineAggregateWrite]>,
}

impl MachineAggregate {
    pub(crate) fn new(
        size: u64,
        alignment: u64,
        writes: impl Into<Box<[MachineAggregateWrite]>>,
    ) -> Self {
        Self {
            size,
            alignment,
            writes: writes.into(),
        }
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn writes(&self) -> &[MachineAggregateWrite] {
        &self.writes
    }
}

/// One target-independent machine operation.
///
/// This domain grows only when a MIR operation has a closed machine meaning. It deliberately does
/// not retain a generic "MIR operation" escape hatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineOperationKind {
    Constant(MachineConstant),
    Load {
        source: MachineAddressId,
    },
    AddressOf {
        source: MachineAddressId,
    },
    Store {
        destination: MachineAddressId,
        value: MachineValueId,
    },
    Unary {
        operation: MachineUnaryOperation,
        operand: MachineValueId,
    },
    Binary {
        operation: MachineBinaryOperation,
        left: MachineValueId,
        right: MachineValueId,
    },
    NumericConversion {
        operand: MachineValueId,
    },
    Comparison(crate::MachineComparison),
    IndexBorrow(crate::MachineIndexBorrow),
    BorrowWeakening {
        source: MachineValueId,
    },
    Aggregate(MachineAggregate),
    EraseCallable(crate::MachineErasedCallable),
    InvokeDrop {
        target: MachineFunctionId,
        place: MachineAddressId,
        allocation: crate::MachineCallAllocation,
    },
    ReportError {
        place: MachineAddressId,
    },
    ReleaseError {
        place: MachineAddressId,
    },
    ReleaseComputation {
        place: MachineAddressId,
    },
    ReleaseErasedCallable {
        place: MachineAddressId,
    },
    /// Releases one compiler-owned mapping after a consuming erased-call adapter has transferred
    /// or destroyed every value stored in it.
    ReleaseMappedStorage {
        pointer: MachineValueId,
        bytes: MachineValueId,
    },
    /// Drives the process-entry computation through its opaque lifecycle and moves the completed
    /// output into caller-owned storage. Only a compiler-generated process root may contain this
    /// operation.
    DriveComputation {
        computation: MachineAddressId,
        destination: Option<MachineAddressId>,
    },
    CreateRegion {
        parent: MachineValueId,
        region: crate::MachineStackId,
    },
    ReleaseRegion {
        region: crate::MachineStackId,
    },
    SetDropFlag {
        flag: MachineDropFlagId,
        initialized: bool,
    },
    Call(MachineCall),
    /// Reads the immutable total element count from the current function's hidden pack input.
    PackLength,
    /// Consumes the next element from the current function's hidden pack input.
    PackNext,
    /// Destroys every element and iterator still owned by the hidden pack input.
    DestroyPack,
}

impl MachineOperationKind {
    /// Whether target lowering may cross an ordinary callable boundary before this operation
    /// completes. Register allocation must preserve every value live after such an operation.
    ///
    /// The match is intentionally exhaustive. Adding a machine operation must classify its call
    /// behavior before the compiler can build, because local optimization and register allocation
    /// both consume this fact.
    #[must_use]
    pub const fn has_call_boundary(&self) -> bool {
        match self {
            Self::Call(_)
            | Self::EraseCallable(_)
            | Self::InvokeDrop { .. }
            | Self::ReportError { .. }
            | Self::ReleaseError { .. }
            | Self::ReleaseComputation { .. }
            | Self::ReleaseErasedCallable { .. }
            | Self::ReleaseMappedStorage { .. }
            | Self::DriveComputation { .. }
            | Self::ReleaseRegion { .. }
            | Self::PackNext
            | Self::DestroyPack => true,
            Self::Constant(_)
            | Self::Load { .. }
            | Self::AddressOf { .. }
            | Self::Store { .. }
            | Self::Unary { .. }
            | Self::Binary { .. }
            | Self::NumericConversion { .. }
            | Self::Comparison(_)
            | Self::IndexBorrow(_)
            | Self::BorrowWeakening { .. }
            | Self::Aggregate(_)
            | Self::CreateRegion { .. }
            | Self::SetDropFlag { .. }
            | Self::PackLength => false,
        }
    }
}

/// One instruction and the optional SSA value it defines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineOperation {
    kind: MachineOperationKind,
    result: Option<MachineValueId>,
}

impl MachineOperation {
    pub(crate) const fn new(kind: MachineOperationKind, result: Option<MachineValueId>) -> Self {
        Self { kind, result }
    }

    #[must_use]
    pub const fn kind(&self) -> &MachineOperationKind {
        &self.kind
    }

    #[must_use]
    pub const fn result(&self) -> Option<MachineValueId> {
        self.result
    }
}

/// One SSA value with the exact stored-layout key selected before machine lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineValue {
    ty: TypeId,
    representation: MachineValueRepresentation,
    definition: MachineValueDefinition,
    storage: MachineValueStorage,
}

impl MachineValue {
    pub(crate) const fn new(
        ty: TypeId,
        representation: MachineValueRepresentation,
        definition: MachineValueDefinition,
    ) -> Self {
        Self {
            ty,
            representation,
            definition,
            storage: MachineValueStorage::Independent,
        }
    }

    pub(crate) const fn with_storage(self, storage: MachineValueStorage) -> Self {
        Self { storage, ..self }
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn representation(self) -> MachineValueRepresentation {
        self.representation
    }

    #[must_use]
    pub const fn definition(self) -> MachineValueDefinition {
        self.definition
    }

    /// The target-independent physical-storage relation proven while the body was frozen.
    ///
    /// Aliased values keep distinct semantic identities and types. Targets may only share their
    /// physical location; they must not substitute one value identity for the other.
    #[must_use]
    pub const fn storage(self) -> MachineValueStorage {
        self.storage
    }
}

/// Whether an SSA value needs independent runtime storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueStorage {
    Independent,
    Alias(MachineValueId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueRepresentation {
    /// One stored value together with Machine's completed transport classification. Targets may
    /// assign physical locations for this class, but must not recover it from `ty` or layout kind.
    Stored {
        size: u64,
        alignment: u64,
        class: crate::MachineValueClass,
    },
    Completion,
    Diverging,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueDefinition {
    BlockParameter {
        block: crate::MachineBlockId,
        position: usize,
    },
    Operation(crate::MachineOperationId),
}
