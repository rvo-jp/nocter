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
    NonDenseAddress(MachineAddressId),
    ProjectedAddress,
    AddressOverflow,
    UnknownStack(nocter_machine::MachineStackId),
    UnknownDropFlag(nocter_machine::MachineDropFlagId),
    DirectMemoryWidth(u64),
    DirectMemoryShape(MachineValueId),
    MemoryShape(MachineValueId),
    AggregateValueShape(MachineValueId),
    AggregateWriteBounds(MachineValueId),
    AggregateStorageAlias(MachineValueId),
    MissingAggregateStaging,
    DirectLaneOffset(u64),
    DirectCopy(MachineOperationId),
    UnsupportedScalar(MachineValueId),
    UnsupportedScalarRepresentation(MachineScalar),
    NonDenseBlock(MachineBlockId),
    UnknownBlock(MachineBlockId),
    EdgeArity(MachineBlockId),
    EdgeTransport(MachineBlockId),
    SwitchSubject,
    Parameters(nocter_machine::MachineLinkageId),
    ParameterTransport(nocter_machine::MachineLinkageId),
    ResultAbi(nocter_machine::MachineLinkageId),
    AllocationEntry(MachineFunctionId),
    MissingResult(MachineOperationId),
    UnsupportedOperation {
        operation: MachineOperationId,
        kind: &'static str,
    },
    TextRepresentation(MachineValueId),
    MemoryValue(MachineValueId),
    ExpectedOneWord(MachineValueId),
    CallArguments(MachineOperationId),
    CallPack(MachineOperationId),
    CallAllocation(MachineOperationId),
    PrimitiveCall(MachineOperationId),
    DropAbi(MachineOperationId),
    MissingIndirectResultPointer,
    ResultTransport(MachineOperationId),
    RootReturn(MachineBlockId),
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

pub(crate) const fn operation_name(
    operation: &nocter_machine::MachineOperationKind,
) -> &'static str {
    use nocter_machine::MachineOperationKind;

    match operation {
        MachineOperationKind::Constant(_) => "constant",
        MachineOperationKind::Load { .. } => "load",
        MachineOperationKind::AddressOf { .. } => "address-of",
        MachineOperationKind::Store { .. } => "store",
        MachineOperationKind::Unary { .. } => "unary",
        MachineOperationKind::Binary { .. } => "binary",
        MachineOperationKind::IntegerConversion { .. } => "integer-conversion",
        MachineOperationKind::Comparison(_) => "comparison",
        MachineOperationKind::IndexBorrow(_) => "index-borrow",
        MachineOperationKind::BorrowWeakening { .. } => "borrow-weakening",
        MachineOperationKind::Aggregate(_) => "aggregate",
        MachineOperationKind::InvokeDrop { .. } => "invoke-drop",
        MachineOperationKind::ReportError { .. } => "report-error",
        MachineOperationKind::CreateRegion { .. } => "create-region",
        MachineOperationKind::ReleaseRegion { .. } => "release-region",
        MachineOperationKind::SetDropFlag { .. } => "set-drop-flag",
        MachineOperationKind::Call(_) => "call",
        MachineOperationKind::PackLength => "pack-length",
        MachineOperationKind::PackNext => "pack-next",
        MachineOperationKind::DestroyPack => "destroy-pack",
    }
}
