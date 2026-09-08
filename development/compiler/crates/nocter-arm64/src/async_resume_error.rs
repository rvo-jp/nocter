use std::fmt;

use crate::{Arm64CodeError, Arm64FrameObjectId};

/// Failure to materialize a native deferred-function resume entry.
#[derive(Debug)]
pub enum Arm64AsyncResumeError {
    ForeignTarget {
        expected: nocter_machine::MachineFunctionId,
        actual: nocter_machine::MachineFunctionId,
    },
    ImmediateTarget(nocter_machine::MachineFunctionId),
    PackTransferUnsupported(nocter_machine::MachineFunctionId),
    MissingState(nocter_machine::MachineBlockId),
    UnknownBlock(nocter_machine::MachineBlockId),
    UnknownValue(nocter_machine::MachineValueId),
    MissingMemoryValue(nocter_machine::MachineValueId),
    UnknownFrameObject(Arm64FrameObjectId),
    MissingFramePointer,
    MissingInterestStaging,
    MissingOutputStaging,
    ContextShape,
    ValueShape(nocter_machine::MachineValueId),
    OutputShape,
    RegisterOverflow,
    InvalidMemoryWidth(u8),
    OffsetOverflow,
    Materialization(crate::Arm64MaterializationError),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64AsyncResumeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 async resume emission failed: {self:?}")
    }
}

impl std::error::Error for Arm64AsyncResumeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Materialization(error) => Some(error),
            Self::Code(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::Arm64MaterializationError> for Arm64AsyncResumeError {
    fn from(error: crate::Arm64MaterializationError) -> Self {
        Self::Materialization(error)
    }
}

impl From<Arm64CodeError> for Arm64AsyncResumeError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}
