use nocter_machine::{MachineOperationId, MachineValueId};

use crate::{Arm64SelectedInstruction, Arm64SelectionContext, Arm64SelectionError};

/// Selects destruction of one owning deferred-computation handle.
///
/// The Machine layer has already proved that `place` stores an async handle. ARM64 selection
/// retains only its physical address; the runtime header is the sole authority for the concrete
/// cancellation entry.
pub(crate) fn select(
    operation: MachineOperationId,
    place: nocter_machine::MachineAddressId,
    result: Option<MachineValueId>,
    context: Arm64SelectionContext<'_>,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if result.is_some() {
        return Err(Arm64SelectionError::AsyncRelease(operation));
    }
    let place = context.addresses().use_address(place, selected)?;
    selected.push(Arm64SelectedInstruction::ReleaseComputation { place });
    Ok(())
}
