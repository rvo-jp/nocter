//! ARM64 instruction and physical-program authority.
//!
//! This crate consumes only [`nocter_machine::MachineProgram`]. Source, semantic, executable, and
//! MIR representations are deliberately absent from its dependency graph.

mod abi;
mod address_code;
mod address_selection;
mod aggregate_selection;
mod allocation_selection;
mod call_selection;
mod code;
mod destruction_selection;
mod encode;
mod frame;
mod frame_access;
mod frame_code;
mod function_frame;
mod identity;
mod instruction;
mod lower;
mod memory_code;
mod memory_parallel_copy;
mod memory_selection;
mod pack_callback;
mod pack_layout;
mod pack_selection;
mod parallel_copy;
mod primitive_memory_code;
mod primitive_memory_selection;
mod primitive_selection;
mod program;
mod register;
mod register_allocation;
mod selected_code;
mod selection;
mod selection_error;
mod structural_selection;
mod switch_code;
mod switch_selection;
mod system_primitive_code;
mod system_primitive_selection;
mod value_plan;

pub use abi::{Arm64AbiRegisterRole, Arm64NocterAbi};
pub use address_selection::{
    Arm64SelectedAddressCalculation, Arm64SelectedAddressPlan, Arm64SelectedAddressRoot,
    Arm64SelectedAddressStep, Arm64SelectedIndex, Arm64SelectedIndexBound,
};
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
pub use pack_callback::{Arm64PackCallbackKey, Arm64PackCallbackKind};
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
pub(crate) use selection::Arm64SelectionContext;
pub use selection::{
    Arm64SelectedBinaryOperation, Arm64SelectedBlock, Arm64SelectedComparisonOperation,
    Arm64SelectedCopy, Arm64SelectedEdge, Arm64SelectedFunction, Arm64SelectedIndexAddressDomain,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress,
    Arm64SelectedMemoryCopy, Arm64SelectedRegister, Arm64SelectedStackAddress,
    Arm64SelectedSwitchCase, Arm64SelectedTerminator, Arm64SelectedUnaryOperation,
};
pub use selection_error::Arm64SelectionError;
pub use value_plan::{Arm64ValuePlan, Arm64ValuePlanError, Arm64ValueStorage};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
