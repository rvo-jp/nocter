//! Target-independent machine-program and ABI layout authority.
//!
//! This crate consumes only validated MIR and the closed toolchain identities retained by that
//! program. It never receives syntax, name resolution, generic requirements, or rendered types.

mod control;
mod identity;
mod layout;
mod linkage;
mod lower;
mod operation;
mod program;
mod storage;
mod target;
mod transport;

pub use control::{
    MachineBlock, MachineBranchTarget, MachineSwitchCase, MachineSwitchValue, MachineTerminator,
};
pub use identity::{
    MachineAddressId, MachineBlockId, MachineDataId, MachineDropFlagId, MachineFunctionId,
    MachineLinkageId, MachineOperationId, MachineStackId, MachineValueId,
};

pub use layout::{
    MachineCaptureLayout, MachineEnumVariantLayout, MachineFieldLayout, MachineLayout,
    MachineLayoutError, MachineLayoutKind, MachineLayoutStore, MachineOutcomeKind,
    MachinePayloadLayout, MachineScalar,
};
pub use linkage::{
    MachineData, MachineDataTable, MachineLinkageEntry, MachineLinkageError, MachineLinkageKey,
    MachineLinkageTable, MachineRootLinkage, MachineTestLinkage,
};
pub use lower::{
    MachineAddressError, MachineAggregateError, MachineProgramError, MachineUnsupportedOperation,
};
pub use operation::{
    MachineAggregate, MachineAggregateWrite, MachineBinaryOperation, MachineCallAllocation,
    MachineConstant, MachineDirectCall, MachineOperation, MachineOperationKind,
    MachineUnaryOperation, MachineValue, MachineValueDefinition,
};
pub use program::{
    MachineBody, MachineFunction, MachineFunctionKind, MachineProgram, MachineProgramRoot,
    MachineTestProgram,
};
pub use storage::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineDropFlag, MachineIndex,
    MachineIndexBound, MachineStackObject, MachineStackPurpose,
};
pub use target::{MachineEndianness, MachineTarget};
pub use transport::{
    MachineAbiError, MachineAbiPlan, MachineArgumentAbi, MachineArgumentLocation,
    MachineCallableAbi, MachinePackAbi, MachineRegisterSpan, MachineResultAbi,
    MachineResultLocation, MachineReturnedValue, MachineStackSlot, MachineValueClass,
};

#[cfg(test)]
mod tests;
