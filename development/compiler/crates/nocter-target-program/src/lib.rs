//! Compiler-owned target capability and target-program construction.
//!
//! Target recognition belongs to `nocter-model`. This crate is the first layer allowed to grant
//! backend capability. Later target-program validation consumes a complete immutable
//! [`ToolchainSnapshot`] instead of reconstructing capability from target or package spellings.

mod body_dependencies;
mod capabilities;
mod closure_instance;
mod drop_instance;
mod entry;
mod executable;
mod instance_key;
mod primitive_contracts;
mod primitive_registry;
mod program;
mod snapshot;
mod test_entry;

pub use body_dependencies::{
    BodyDependencyError, CheckedBodyDependencies, CheckedDestruction, PreparedBorrow,
    collect_body_dependencies,
};
pub use capabilities::{
    ExecutableWriterIdentity, TargetAbiIdentity, TargetBackendIdentity, TargetUnavailable,
};
pub use closure_instance::{ClosureInstanceKey, ClosureInstanceKeyError};
pub use drop_instance::{DropInstanceKey, DropInstanceKeyError};
pub use entry::{
    EntryContractRule, EntrySelectionError, ExecutableEntry, ProcessResultContract,
    ProcessSuccessType, select_executable_entry,
};
pub use executable::{
    ExecutableBody, ExecutableBorrowEdge, ExecutableCallableInvocation, ExecutableClosureCapture,
    ExecutableClosureEdge, ExecutableClosureLayout, ExecutableDispatchPlan, ExecutableDispatchStep,
    ExecutableDropEdge, ExecutableInput, ExecutableInputSource, ExecutableItem, ExecutableItemKey,
    ExecutablePackInput, ExecutablePrimitiveCall, ExecutableProgram, ExecutableProgramError,
    ExecutableRoot, ExecutableSignature, ExecutableTestCase, ExecutableTypeEdge,
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
