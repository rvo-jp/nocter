use nocter_runtime_contract::RuntimeTypeTable;

use crate::{MachineAbiPlan, MachineDataTable, MachineDestructionTable, MachineLayoutStore};

/// Immutable whole-program authorities shared by every source-body lowering operation.
#[derive(Clone, Copy)]
pub(super) struct ProgramLoweringContext<'a> {
    pub(super) types: &'a RuntimeTypeTable,
    pub(super) layouts: &'a MachineLayoutStore,
    pub(super) abi: &'a MachineAbiPlan,
    pub(super) data: &'a MachineDataTable,
    pub(super) functions: crate::function_domain::MachineFunctionDomain<'a>,
    pub(super) destructions: &'a MachineDestructionTable,
}
