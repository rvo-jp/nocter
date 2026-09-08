use nocter_machine::{MachineAddressId, MachineFunctionKind, MachineOperationId, MachineValueId};

use crate::{
    Arm64SelectedInstruction, Arm64SelectedMemoryAddress, Arm64SelectionContext,
    Arm64SelectionError,
};

/// Selects the compiler-owned process boundary that drives one deferred entry computation.
pub(crate) fn select(
    context: Arm64SelectionContext<'_>,
    operation: MachineOperationId,
    computation: MachineAddressId,
    destination: Option<MachineAddressId>,
    result: Option<MachineValueId>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let is_process_root = context
        .program()
        .function(context.owner())
        .is_some_and(|function| matches!(function.kind(), MachineFunctionKind::ProcessRoot));
    if result.is_some() || !is_process_root {
        return Err(Arm64SelectionError::AsyncDrive(operation));
    }
    let computation = context.addresses().use_address(computation, selected)?;
    let destination = destination
        .map(|destination| context.addresses().use_address(destination, selected))
        .transpose()?;
    if !matches!(computation, Arm64SelectedMemoryAddress::Stack(_))
        || destination
            .is_some_and(|destination| !matches!(destination, Arm64SelectedMemoryAddress::Stack(_)))
    {
        return Err(Arm64SelectionError::AsyncDrive(operation));
    }
    selected.push(Arm64SelectedInstruction::DriveComputation {
        computation,
        destination,
    });
    Ok(())
}
