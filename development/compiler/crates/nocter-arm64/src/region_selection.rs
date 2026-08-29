use nocter_machine::{MachineOperationId, MachineStackId, MachineStackPurpose, MachineValueId};

use crate::{
    Arm64SelectedInstruction, Arm64SelectionContext, Arm64SelectionError, Arm64ValueStorage,
};

/// Closes creation over the exact frame object that owns the lexical context.
pub(crate) fn select_create(
    operation: MachineOperationId,
    parent: MachineValueId,
    region: MachineStackId,
    result: Option<MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let Some(Arm64ValueStorage::Direct(registers)) = context.values().value(parent) else {
        return Err(Arm64SelectionError::RegionOperation(operation));
    };
    let [parent] = registers.as_ref() else {
        return Err(Arm64SelectionError::RegionOperation(operation));
    };
    if result.is_some() {
        return Err(Arm64SelectionError::RegionOperation(operation));
    }
    selected.push(Arm64SelectedInstruction::CreateRegion {
        region: region_object(operation, region, context)?,
        parent: crate::Arm64SelectedRegister::Virtual(*parent),
    });
    Ok(())
}

/// Closes release over the same frame object. Release is a call boundary because its target-owned
/// implementation invokes the operating-system unmap service.
pub(crate) fn select_release(
    operation: MachineOperationId,
    region: MachineStackId,
    result: Option<MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if result.is_some() {
        return Err(Arm64SelectionError::RegionOperation(operation));
    }
    selected.push(Arm64SelectedInstruction::ReleaseRegion {
        region: region_object(operation, region, context)?,
    });
    Ok(())
}

fn region_object(
    operation: MachineOperationId,
    region: MachineStackId,
    context: Arm64SelectionContext<'_>,
) -> Result<crate::Arm64FrameObjectId, Arm64SelectionError> {
    let function = context
        .program()
        .function(context.owner())
        .ok_or(Arm64SelectionError::UnknownFunction(context.owner()))?;
    if !matches!(
        function
            .body()
            .stack(region)
            .map(nocter_machine::MachineStackObject::purpose),
        Some(MachineStackPurpose::Region)
    ) {
        return Err(Arm64SelectionError::RegionOperation(operation));
    }
    context
        .frame()
        .stack_object(region)
        .ok_or(Arm64SelectionError::RegionOperation(operation))
}
