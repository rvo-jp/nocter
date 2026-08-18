//! ARM64 instruction and physical-program authority.
//!
//! This crate consumes only [`nocter_machine::MachineProgram`]. Source, semantic, executable, and
//! MIR representations are deliberately absent from its dependency graph.

mod abi;
mod code;
mod encode;
mod frame;
mod frame_code;
mod identity;
mod instruction;
mod program;
mod register;
mod register_allocation;

pub use abi::{Arm64AbiRegisterRole, Arm64NocterAbi};
pub use code::{Arm64Code, Arm64CodeBuilder, Arm64CodeError, Arm64LabelId};
pub use encode::Arm64EncodingError;
pub use frame::{
    Arm64FrameLayout, Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObject,
    Arm64FrameObjectId, Arm64SavedRegister,
};
pub use frame_code::Arm64FrameCode;
pub use identity::{Arm64DataId, Arm64FunctionId};
pub use instruction::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64MoveWide, Arm64Shift,
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

#[cfg(test)]
mod tests;
