use nocter_model::TypeId;

use crate::{
    MachineAddressId, MachineCall, MachineDataId, MachineDropFlagId, MachineFunctionId,
    MachineValueId,
};

/// A fully materialized constant. Text refers to the program's canonical static-data table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineConstant {
    Bool(bool),
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
    Tag { offset: u64, value: u8 },
    Value { offset: u64, value: MachineValueId },
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
    IntegerConversion {
        operand: MachineValueId,
    },
    Comparison(crate::MachineComparison),
    IndexBorrow(crate::MachineIndexBorrow),
    BorrowWeakening {
        source: MachineValueId,
    },
    Aggregate(MachineAggregate),
    InvokeDrop {
        target: MachineFunctionId,
        place: MachineAddressId,
    },
    ReportError {
        error: MachineValueId,
    },
    CreateRegion {
        parent: MachineValueId,
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
        }
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineValueRepresentation {
    Stored { size: u64, alignment: u64 },
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
