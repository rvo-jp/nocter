use crate::{MachineScalar, MachineValueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineComparisonOperation {
    Equal,
    Less,
}

/// Exact stored representation inspected by a compiler-provided comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineComparisonRepresentation {
    Scalar(MachineScalar),
    Tag { offset: u64 },
}

/// One primitive comparison over two readonly-borrow values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineComparison {
    operation: MachineComparisonOperation,
    representation: MachineComparisonRepresentation,
    left: MachineValueId,
    right: MachineValueId,
}

impl MachineComparison {
    pub(crate) const fn new(
        operation: MachineComparisonOperation,
        representation: MachineComparisonRepresentation,
        left: MachineValueId,
        right: MachineValueId,
    ) -> Self {
        Self {
            operation,
            representation,
            left,
            right,
        }
    }

    #[must_use]
    pub const fn operation(self) -> MachineComparisonOperation {
        self.operation
    }

    #[must_use]
    pub const fn representation(self) -> MachineComparisonRepresentation {
        self.representation
    }

    #[must_use]
    pub const fn left(self) -> MachineValueId {
        self.left
    }

    #[must_use]
    pub const fn right(self) -> MachineValueId {
        self.right
    }
}

/// Bounds and address extraction needed by one built-in indexing operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineIndexDomain {
    Fixed {
        length: u64,
        stride: u64,
    },
    View {
        pointer_offset: u64,
        length_offset: u64,
        stride: u64,
    },
}

/// One checked built-in index that produces a borrow to an element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineIndexBorrow {
    receiver: MachineValueId,
    index: MachineValueId,
    domain: MachineIndexDomain,
}

impl MachineIndexBorrow {
    pub(crate) const fn new(
        receiver: MachineValueId,
        index: MachineValueId,
        domain: MachineIndexDomain,
    ) -> Self {
        Self {
            receiver,
            index,
            domain,
        }
    }

    #[must_use]
    pub const fn receiver(self) -> MachineValueId {
        self.receiver
    }

    #[must_use]
    pub const fn index(self) -> MachineValueId {
        self.index
    }

    #[must_use]
    pub const fn domain(self) -> MachineIndexDomain {
        self.domain
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineStructuralError {
    InvalidSignature,
    InvalidRepresentation,
}
