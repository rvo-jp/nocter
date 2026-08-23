use std::collections::BTreeMap;

use nocter_model::ExecutableItemId;
use nocter_runtime_contract::RuntimeTypeTable;

use crate::{
    MachineAbiPlan, MachineDataTable, MachineDestructionId, MachineDestructionTable,
    MachineFunctionId, MachineLayoutStore,
};

/// Immutable whole-program authorities shared by every source-body lowering operation.
#[derive(Clone, Copy)]
pub(super) struct ProgramLoweringContext<'a> {
    pub(super) types: &'a RuntimeTypeTable,
    pub(super) layouts: &'a MachineLayoutStore,
    pub(super) abi: &'a MachineAbiPlan,
    pub(super) data: &'a MachineDataTable,
    pub(super) functions: &'a BTreeMap<ExecutableItemId, MachineFunctionId>,
    pub(super) destructions: &'a MachineDestructionTable,
    pub(super) destruction_functions: &'a BTreeMap<MachineDestructionId, MachineFunctionId>,
}
