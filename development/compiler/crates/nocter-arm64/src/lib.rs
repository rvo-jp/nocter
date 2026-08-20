//! ARM64 instruction and physical-program authority.
//!
//! This crate consumes only [`nocter_machine::MachineProgram`]. Source, semantic, executable, and
//! MIR representations are deliberately absent from its dependency graph.

mod abi;
mod code;
mod encode;
mod frame;
mod frame_access;
mod frame_code;
mod function_frame;
mod identity;
mod instruction;
mod lower;
mod memory_code;
mod memory_selection;
mod pack_layout;
mod parallel_copy;
mod program;
mod register;
mod register_allocation;
mod selected_code;
mod selection;
mod selection_error;
mod value_plan;

pub use abi::{Arm64AbiRegisterRole, Arm64NocterAbi};
pub use code::{Arm64Code, Arm64CodeBuilder, Arm64CodeError, Arm64LabelId};
pub use encode::Arm64EncodingError;
pub use frame::{
    Arm64FrameLayout, Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObject,
    Arm64FrameObjectId, Arm64SavedRegister,
};
pub use frame_code::Arm64FrameCode;
pub use function_frame::{
    Arm64AllocationContextFrame, Arm64FunctionFrame, Arm64FunctionFrameError, Arm64PackFrame,
};
pub use identity::{Arm64DataId, Arm64FunctionId};
pub use instruction::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64MoveWide, Arm64Shift,
};
pub use lower::Arm64LoweringError;
pub use pack_layout::{
    Arm64PackDescriptorLayout, Arm64PackLayoutError, Arm64PackSegmentLayout, Arm64PackStateLayout,
};
pub use program::{
    Arm64DataAddressFixup, Arm64DataRange, Arm64FunctionRange, Arm64Program, Arm64ProgramBuilder,
    Arm64ProgramError,
};
pub use register::{
    Arm64AddSubtractDestination, Arm64BaseRegister, Arm64DataRegister, Arm64Register,
};
pub use register_allocation::{
    Arm64AllocatedLocation, Arm64RegisterAllocation, Arm64RegisterAllocationBuilder,
    Arm64RegisterAllocationError, Arm64SpillSlotId, Arm64VirtualRegister,
};
pub use selected_code::Arm64MaterializationError;
pub use selection::{
    Arm64SelectedBinaryOperation, Arm64SelectedBlock, Arm64SelectedComparisonOperation,
    Arm64SelectedCopy, Arm64SelectedEdge, Arm64SelectedFunction, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedRegister, Arm64SelectedStackAddress,
    Arm64SelectedTerminator, Arm64SelectedUnaryOperation,
};
pub use selection_error::Arm64SelectionError;
pub use value_plan::{Arm64ValuePlan, Arm64ValuePlanError, Arm64ValueStorage};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
