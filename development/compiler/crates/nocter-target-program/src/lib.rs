//! Compiler-owned target capability and target-program construction.
//!
//! Target recognition belongs to `nocter-model`. This crate is the first layer allowed to grant
//! backend capability. Later target-program validation consumes a complete immutable
//! [`ToolchainSnapshot`] instead of reconstructing capability from target or package spellings.

mod capabilities;
mod primitive_contracts;
mod primitive_registry;
mod program;
mod snapshot;

pub use capabilities::{
    ExecutableWriterIdentity, TargetAbiIdentity, TargetBackendIdentity, TargetUnavailable,
};
pub use primitive_contracts::{
    PrimitiveContractError, PrimitiveContractRule, PrimitiveRegistryValidationError,
};
pub use primitive_registry::{
    PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole,
};
pub use program::{TargetProgram, TargetProgramError};
pub use snapshot::ToolchainSnapshot;
