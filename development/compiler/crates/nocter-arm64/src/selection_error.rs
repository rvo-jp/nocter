use std::fmt;

use nocter_machine::{
    MachineAddressId, MachineBlockId, MachineDataId, MachineFunctionId, MachineOperationId,
    MachineScalar, MachineValueId,
};

use crate::{Arm64FunctionFrameError, Arm64ValuePlanError};

#[derive(Debug)]
pub enum Arm64SelectionError {
    UnknownFunction(MachineFunctionId),
    NonCallableTarget(MachineFunctionId),
    UnknownOperation {
        function: MachineFunctionId,
        operation: MachineOperationId,
    },
    UnknownValue(MachineValueId),
    UnknownData(MachineDataId),
    UnknownAddress(MachineAddressId),
    NonStackAddress,
    ProjectedAddress,
    AddressOverflow,
    UnknownStack(nocter_machine::MachineStackId),
    DirectMemoryWidth(u64),
    DirectMemoryShape(MachineValueId),
    DirectLaneOffset(u64),
    UnsupportedScalar(MachineValueId),
    UnsupportedScalarRepresentation(MachineScalar),
    NonDenseBlock(MachineBlockId),
    UnknownBlock(MachineBlockId),
    EdgeArity(MachineBlockId),
    EdgeTransport(MachineBlockId),
    Parameters(nocter_machine::MachineLinkageId),
    ParameterTransport(nocter_machine::MachineLinkageId),
    MissingResult(MachineOperationId),
    UnsupportedOperation {
        operation: MachineOperationId,
        kind: &'static str,
    },
    UnsupportedTerminator(MachineBlockId),
    TextRepresentation(MachineValueId),
    MemoryValue(MachineValueId),
    ExpectedOneWord(MachineValueId),
    CallArguments(MachineOperationId),
    CallPack(MachineOperationId),
    CallAllocation(MachineOperationId),
    PrimitiveCall(MachineOperationId),
    IndirectResult(MachineOperationId),
    ResultTransport(MachineOperationId),
    RootReturn(MachineBlockId),
    IndirectReturn(MachineBlockId),
    ReturnTransport(MachineBlockId),
    RegisterOverflow,
    Allocation(nocter_machine::MachineAllocationError),
    ValuePlan(Arm64ValuePlanError),
    Frame(Arm64FunctionFrameError),
}

impl fmt::Display for Arm64SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 instruction selection failed: {self:?}")
    }
}

impl std::error::Error for Arm64SelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::ValuePlan(error) => Some(error),
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<nocter_machine::MachineAllocationError> for Arm64SelectionError {
    fn from(error: nocter_machine::MachineAllocationError) -> Self {
        Self::Allocation(error)
    }
}

impl From<Arm64ValuePlanError> for Arm64SelectionError {
    fn from(error: Arm64ValuePlanError) -> Self {
        Self::ValuePlan(error)
    }
}

impl From<Arm64FunctionFrameError> for Arm64SelectionError {
    fn from(error: Arm64FunctionFrameError) -> Self {
        Self::Frame(error)
    }
}
