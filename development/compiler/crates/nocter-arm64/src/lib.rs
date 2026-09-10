//! ARM64 instruction and physical-program authority.
//!
//! This crate consumes [`nocter_machine::MachineProgram`] plus the closed primitive roles in
//! [`nocter_runtime_contract`]. Source, semantic, executable, and MIR representations are
//! deliberately absent from its dependency graph.

mod abi;
mod address_code;
mod address_selection;
mod aggregate_selection;
mod allocation_selection;
mod async_activation;
mod async_cancel_code;
mod async_cancellation;
mod async_constructor_code;
mod async_consume_code;
mod async_drive_code;
mod async_drive_selection;
mod async_frame;
mod async_function;
mod async_interest_code;
mod async_join_code;
mod async_pack_capture_code;
mod async_primitive_targets;
mod async_release_code;
mod async_release_selection;
mod async_resume_code;
mod async_resume_error;
mod async_wait_code;
mod async_wait_frame;
mod async_wait_timeout_code;
mod call_selection;
mod code;
mod darwin_block;
mod darwin_kernel_abi;
mod darwin_memory_code;
mod darwin_network_adapter;
mod darwin_network_callback;
mod darwin_network_channel;
mod darwin_network_connection;
mod darwin_network_connection_address;
mod darwin_network_connection_event;
mod darwin_network_connection_transfer;
mod darwin_network_owner;
mod darwin_network_owner_creation;
mod darwin_network_owner_event;
mod darwin_network_owner_lifecycle;
mod darwin_network_primitive_targets;
mod destruction_selection;
mod encode;
mod error_code;
mod error_selection;
mod floating_code;
mod floating_encoding;
mod floating_parallel_copy;
mod frame;
mod frame_access;
mod frame_code;
mod function_frame;
mod function_targets;
mod identity;
mod instruction;
mod lower;
mod memory_code;
mod memory_parallel_copy;
mod memory_selection;
mod object_layout;
mod pack_allocation_code;
mod pack_callback;
mod pack_layout;
mod pack_selection;
mod parallel_copy;
mod parallel_copy_schedule;
mod primitive_memory_code;
mod primitive_memory_selection;
mod primitive_selection;
mod primitive_targets;
mod process_code;
mod process_layout;
mod process_selection;
mod program;
mod region_code;
mod region_layout;
mod region_selection;
mod register;
mod register_allocation;
mod runtime_trap;
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
pub use async_activation::Arm64AsyncActivationPlanError;
pub use async_cancel_code::Arm64AsyncCancelError;
pub use async_cancellation::Arm64AsyncCancellationPlanError;
pub use async_constructor_code::Arm64AsyncConstructorError;
pub use async_consume_code::Arm64AsyncConsumeError;
pub use async_frame::{
    Arm64AsyncFrameField, Arm64AsyncFrameLayout, Arm64AsyncFrameLayoutError,
    Arm64AsyncSuspensionTag,
};
pub use async_function::{
    Arm64AsyncFunctionPlan, Arm64AsyncFunctionPlanError, Arm64AsyncPackCapture,
    Arm64AsyncParameterCapture,
};
pub use async_primitive_targets::{
    Arm64AsyncInterestLifecycleTargets, Arm64AsyncJoinTargets, Arm64AsyncPrimitiveTargets,
};
pub use async_resume_error::Arm64AsyncResumeError;
pub use async_wait_frame::Arm64AsyncWaitFrame;
pub use code::{Arm64Code, Arm64CodeBuilder, Arm64CodeError, Arm64LabelId};
pub use darwin_block::{
    Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError,
    add_darwin_pointer_capture_block_descriptor, load_darwin_stack_block_address,
    materialize_darwin_pointer_capture_stack_block,
};
pub use darwin_network_adapter::Arm64DarwinNetworkAdapterImports;
pub use darwin_network_callback::{
    Arm64DarwinNetworkCallbackError, add_darwin_network_completion_callback,
    add_darwin_network_state_callback,
};
pub use darwin_network_channel::{
    Arm64DarwinNetworkChannelError, Arm64DarwinNetworkChannelImports,
    emit_darwin_network_event_receive, emit_darwin_network_event_receive_to_pointer,
    emit_darwin_network_event_send,
};
pub use darwin_network_connection::{
    Arm64DarwinNetworkConnectionError, Arm64DarwinNetworkConnectionTargets,
    add_darwin_plain_connection_targets,
};
pub use darwin_network_connection_address::{
    Arm64DarwinNetworkConnectionAddressError, Arm64DarwinNetworkConnectionAddressTargets,
};
pub use darwin_network_connection_event::{
    Arm64DarwinNetworkConnectionEventError, Arm64DarwinNetworkConnectionEventTargets,
};
pub use darwin_network_connection_transfer::{
    Arm64DarwinNetworkConnectionTransferError, Arm64DarwinNetworkConnectionTransferTargets,
};
pub use darwin_network_owner::{
    Arm64DarwinNetworkOwnerError, Arm64DarwinNetworkOwnerResources,
    emit_darwin_network_owner_guard, emit_darwin_network_owner_initialize,
    emit_darwin_network_owner_release, emit_darwin_network_owner_transition,
};
pub use darwin_network_owner_event::Arm64DarwinNetworkOwnerEventError;
pub use darwin_network_owner_lifecycle::{
    Arm64DarwinNetworkOwnerLifecycleError, Arm64DarwinNetworkOwnerLifecycleTargets,
};
pub use darwin_network_primitive_targets::{
    Arm64DarwinNetworkPrimitive, Arm64DarwinNetworkPrimitiveError,
    Arm64DarwinNetworkPrimitiveTargets,
};
pub use encode::Arm64EncodingError;
pub use frame::{
    Arm64FrameLayout, Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObject,
    Arm64FrameObjectId, Arm64SavedRegister,
};
pub use frame_code::Arm64FrameCode;
pub use function_frame::{
    Arm64AllocationContextFrame, Arm64FunctionFrame, Arm64FunctionFrameError, Arm64PackFrame,
    Arm64ProcessContextFrame,
};
pub use function_targets::{
    Arm64AsyncFunctionTargets, Arm64FunctionTarget, Arm64FunctionTargets, Arm64FunctionTargetsError,
};
pub use identity::{Arm64DataId, Arm64FunctionId};
pub use instruction::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64DataSize, Arm64FloatBinary, Arm64FloatRounding,
    Arm64Instruction, Arm64LoadStoreSize, Arm64Logical, Arm64MoveWide, Arm64Shift,
    Arm64SystemRegister,
};
pub use lower::{Arm64LoweringError, Arm64TestExecutable, Arm64TestSuite};
pub use pack_callback::{Arm64PackCallbackKey, Arm64PackCallbackKind};
pub use pack_layout::{
    Arm64PackDescriptorLayout, Arm64PackLayoutError, Arm64PackSegmentLayout, Arm64PackStateLayout,
};
pub use program::{
    Arm64DataAddressFixup, Arm64DataImportId, Arm64DataPointerFixup, Arm64DataRange,
    Arm64FunctionImportId, Arm64FunctionRange, Arm64Program, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64RelocatedSections, Arm64RuntimeImport,
};
pub use register::{
    Arm64AddSubtractDestination, Arm64BaseRegister, Arm64DataRegister, Arm64FloatRegister,
    Arm64Register,
};
pub use register_allocation::{
    Arm64AllocatedLocation, Arm64RegisterAllocation, Arm64RegisterAllocationBuilder,
    Arm64RegisterAllocationError, Arm64RegisterClass, Arm64SpillSlotId, Arm64VirtualRegister,
};
pub use selected_code::Arm64MaterializationError;
pub(crate) use selection::Arm64SelectionContext;
pub use selection::{
    Arm64SelectedBinaryOperation, Arm64SelectedBlock, Arm64SelectedComparisonOperation,
    Arm64SelectedCopy, Arm64SelectedEdge, Arm64SelectedFloatComparisonOperation,
    Arm64SelectedFloatCopy, Arm64SelectedFloatRegister, Arm64SelectedFunction,
    Arm64SelectedIndexAddressDomain, Arm64SelectedInstruction, Arm64SelectedLoadExtension,
    Arm64SelectedMemoryAddress, Arm64SelectedMemoryCopy, Arm64SelectedRegister,
    Arm64SelectedStackAddress, Arm64SelectedSwitchCase, Arm64SelectedTerminator,
    Arm64SelectedUnaryOperation,
};
pub use selection_error::Arm64SelectionError;
pub use value_plan::{Arm64ValuePlan, Arm64ValuePlanError, Arm64ValueStorage};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
