//! Closed private MIR enters the common executable Machine body and native pipeline.
mod executable;
use nocter_mir::value_access as mir;
use nocter_model::AccessInstanceId;

/// Lower closed private MIR into the common executable Machine program.
/// # Errors
/// Rejects an unsupported process-root signature or inconsistent closed representation.
pub fn lower(program: &mir::Program) -> Result<crate::MachineProgram, ExecutableFailure> {
    executable::program(program)
}

#[derive(Debug)]
pub enum ExecutableFailure {
    Layout(crate::MachineLayoutError),
    Abi(crate::MachineAbiError),
    InvalidBody(AccessInstanceId),
    InvalidRoot,
    DataLimit(nocter_model::TypeId),
    UnsupportedArrayCleanup(nocter_model::TypeId),
    Optimization(crate::MachineOptimizationError),
    Dataflow(crate::MachineDataflowError),
    Context(crate::MachineContextError),
}
