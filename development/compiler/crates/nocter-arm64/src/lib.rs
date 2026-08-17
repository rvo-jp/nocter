//! ARM64 instruction and physical-program authority.
//!
//! This crate consumes only [`nocter_machine::MachineProgram`]. Source, semantic, executable, and
//! MIR representations are deliberately absent from its dependency graph.

mod encode;
mod instruction;
mod register;

pub use encode::Arm64EncodingError;
pub use instruction::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Logical, Arm64MoveWide, Arm64Shift,
};
pub use register::{
    Arm64AddSubtractDestination, Arm64BaseRegister, Arm64DataRegister, Arm64Register,
};

#[cfg(test)]
mod tests;
