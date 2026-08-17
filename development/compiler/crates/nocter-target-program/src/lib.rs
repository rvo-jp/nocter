//! Compiler-owned target capability and target-program construction.
//!
//! Target recognition belongs to `nocter-model`. This crate is the first layer allowed to grant
//! backend capability. Later target-program validation consumes a complete immutable
//! [`ToolchainSnapshot`] instead of reconstructing capability from target or package spellings.

mod body_dependencies;
mod capabilities;
mod entry;
mod instance_key;
mod primitive_contracts;
mod primitive_registry;
mod program;
mod snapshot;
mod test_entry;

pub use body_dependencies::{
    BodyDependencyError, CheckedBodyDependencies, collect_body_dependencies,
};
pub use capabilities::{
    ExecutableWriterIdentity, TargetAbiIdentity, TargetBackendIdentity, TargetUnavailable,
};
pub use entry::{
    EntryContractRule, EntrySelectionError, ExecutableEntry, ProcessResultContract,
    ProcessSuccessType, select_executable_entry,
};
pub use instance_key::{CallableInstanceKey, CallableInstanceKeyError};
pub use primitive_contracts::{
    PrimitiveContractError, PrimitiveContractRule, PrimitiveRegistryValidationError,
};
pub use primitive_registry::{
    PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole,
};
pub use program::{TargetProgram, TargetProgramError};
pub use snapshot::ToolchainSnapshot;
pub use test_entry::{SelectedTest, SelectedTestTarget, TestSelectionError, select_test_target};
