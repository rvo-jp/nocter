use nocter_runtime_contract::RuntimeTypeTable;

use crate::{MachineAbiPlan, MachineLayoutPlan};

/// Immutable whole-program authorities shared by every source-body lowering operation.
#[derive(Clone, Copy)]
pub(super) struct ProgramLoweringContext<'a> {
    pub(super) types: &'a RuntimeTypeTable,
    pub(super) layouts: &'a MachineLayoutPlan,
    pub(super) abi: &'a MachineAbiPlan,
    pub(super) data: &'a crate::data::MachineDataPlan,
    pub(super) functions: crate::function_domain::MachineFunctionDomain<'a>,
    pub(super) destructions: &'a crate::destruction_table::MachineDestructionPlanTable,
}
