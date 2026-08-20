//! Target-independent machine-program and ABI layout authority.
//!
//! This crate consumes only validated MIR and the closed toolchain identities retained by that
//! program. It never receives syntax, name resolution, generic requirements, or rendered types.

mod allocation;
mod call;
mod control;
mod dataflow;
mod destruction;
mod destruction_table;
mod generated_destruction;
mod identity;
mod layout;
mod linkage;
mod lower;
mod operation;
mod pack;
mod primitive_dependency;
mod program;
mod storage;
mod structural;
mod target;
mod transport;

pub use allocation::{MachineAllocationError, MachineAllocationPlan, MachineAllocationRequirement};
pub use call::{MachineCall, MachineCallAllocation, MachineCallTarget, MachinePrimitiveTarget};
pub use control::{
    MachineBlock, MachineBranchTarget, MachineSwitchCase, MachineSwitchValue, MachineTerminator,
};
pub use dataflow::{
    MachineBlockDataflow, MachineDataflowError, MachineFunctionDataflow, MachineOperationDataflow,
};
pub use destruction::{
    MachineDestructionCapture, MachineDestructionError, MachineDestructionField,
    MachineDestructionKind, MachineDestructionPayload, MachineDestructionPlan,
    MachineDestructionVariant,
};
pub use destruction_table::{MachineDestruction, MachineDestructionTable};
pub use identity::{
    MachineAddressId, MachineBlockId, MachineDataId, MachineDestructionId, MachineDropFlagId,
    MachineFunctionId, MachineLinkageId, MachineOperationId, MachinePackId, MachineStackId,
    MachineValueId,
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
pub use lower::{MachineAddressError, MachineAggregateError, MachineProgramError};
pub use nocter_target_program::PrimitiveRole;
pub use operation::{
    MachineAggregate, MachineAggregateWrite, MachineBinaryOperation, MachineConstant,
    MachineOperation, MachineOperationKind, MachineUnaryOperation, MachineValue,
    MachineValueDefinition, MachineValueRepresentation,
};
pub use pack::{
    MachinePack, MachinePackContribution, MachinePackNext, MachinePackSegment, MachinePackSpread,
};
pub use primitive_dependency::MachinePrimitiveDependency;
pub use program::{
    MachineBody, MachineFunction, MachineFunctionKind, MachineProgram, MachineProgramRoot,
    MachineTestProgram,
};
pub use storage::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineDropFlag, MachineIndex,
    MachineIndexBound, MachineStackObject, MachineStackPurpose,
};
pub use structural::{
    MachineComparison, MachineComparisonOperation, MachineComparisonRepresentation,
    MachineIndexBorrow, MachineIndexDomain, MachineStructuralError,
};
pub use target::{MachineEndianness, MachineTarget};
pub use transport::{
    MachineAbiError, MachineAbiPlan, MachineArgumentAbi, MachineArgumentLocation,
    MachineCallableAbi, MachinePackAbi, MachineRegisterSpan, MachineResultAbi,
    MachineResultLocation, MachineReturnedValue, MachineStackSlot, MachineValueClass,
};

#[cfg(test)]
mod tests;
