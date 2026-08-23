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
mod program;
mod snapshot;
mod test_entry;

pub use body_dependencies::{
    BodyDependencyError, CheckedBodyDependencies, CheckedDestruction, PreparedBorrow,
    collect_body_dependencies,
};
pub use capabilities::{ExecutableWriterIdentity, TargetBackendIdentity, TargetUnavailable};
pub use closure_instance::{ClosureInstanceKey, ClosureInstanceKeyError};
pub use drop_instance::{DropInstanceKey, DropInstanceKeyError};
pub use entry::{
    EntryContractRule, EntrySelectionError, ExecutableEntry, ProcessResultContract,
    ProcessSuccessType, select_executable_entry,
};
pub use executable::{
    ExecutableArgumentPackPlan, ExecutableBody, ExecutableBorrowEdge, ExecutableCallableInvocation,
    ExecutableClosureCapture, ExecutableClosureEdge, ExecutableClosureLayout,
    ExecutableDispatchPlan, ExecutableDispatchStep, ExecutableDropEdge, ExecutableInput,
    ExecutableInputSource, ExecutableItem, ExecutableItemKey, ExecutableOpaqueReceiver,
    ExecutablePackInput, ExecutablePackSegment, ExecutablePackSpread, ExecutablePrimitiveCall,
    ExecutablePrimitiveDependency, ExecutableProgram, ExecutableProgramError, ExecutableRoot,
    ExecutableSequencePlan, ExecutableSignature, ExecutableTestCase, ExecutableTypeEdge,
};
pub use instance_key::{CallableInstanceKey, CallableInstanceKeyError};
use nocter_runtime_contract::{
    PrimitiveRegistry, PrimitiveRole, RuntimeAbiIdentity, RuntimeTypeRepresentationTable,
};
pub use primitive_contracts::{
    PrimitiveContractError, PrimitiveContractRule, PrimitiveRegistryValidationError,
};
pub use program::{TargetProgram, TargetProgramError};
pub use snapshot::ToolchainSnapshot;
pub use test_entry::{
    SelectedTest, SelectedTestTarget, TestCaseSelectionError, TestSelectionError, select_test_case,
    select_test_target,
};
