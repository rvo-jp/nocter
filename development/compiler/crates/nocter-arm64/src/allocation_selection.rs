use nocter_machine::{
    MachineAllocationRequirement, MachineCall, MachineCallAllocation, MachineFunctionId,
};

use crate::{
    Arm64AllocationContextFrame, Arm64FunctionFrame, Arm64NocterAbi, Arm64SelectedAddressPlan,
    Arm64SelectedInstruction, Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectionError,
};

/// Establishes or saves the allocation-context pointer before any function input can be clobbered.
pub(crate) fn select_entry(
    program: &nocter_machine::MachineProgram,
    function: MachineFunctionId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let requirement = program
        .allocation()
        .get(function)
        .ok_or(Arm64SelectionError::AllocationEntry(function))?;
    match (requirement, frame.allocation_context()) {
        (MachineAllocationRequirement::None, Arm64AllocationContextFrame::None) => Ok(()),
        (
            MachineAllocationRequirement::ProgramRoot,
            Arm64AllocationContextFrame::ProgramRoot(object),
        ) => {
            selected.push(Arm64SelectedInstruction::ZeroStack {
                destination: Arm64SelectedStackAddress::FrameObject { object, offset: 0 },
                bytes: context_size(),
            });
            Ok(())
        }
        (
            MachineAllocationRequirement::Incoming,
            Arm64AllocationContextFrame::IncomingPointer(object),
        ) => {
            selected.push(Arm64SelectedInstruction::StoreMemory {
                bytes: word_bytes(),
                destination: Arm64SelectedMemoryAddress::Stack(
                    Arm64SelectedStackAddress::FrameObject { object, offset: 0 },
                ),
                source: Arm64SelectedRegister::Fixed(Arm64NocterAbi::allocation_context_register()),
            });
            Ok(())
        }
        _ => Err(Arm64SelectionError::AllocationEntry(function)),
    }
}

/// Materializes the exact inherited or explicit context selected by one machine call into `x9`.
pub(crate) fn select_call(
    program: &nocter_machine::MachineProgram,
    operation: nocter_machine::MachineOperationId,
    call: &MachineCall,
    frame: &Arm64FunctionFrame,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let requires_context = program.allocation().call_requires_context(call)?;
    match (requires_context, call.allocation()) {
        (false, MachineCallAllocation::Inherit) => Ok(()),
        (false, MachineCallAllocation::Explicit(_)) => {
            Err(Arm64SelectionError::CallAllocation(operation))
        }
        (true, MachineCallAllocation::Inherit) => select_inherited(operation, frame, selected),
        (true, MachineCallAllocation::Explicit(address)) => {
            let source = addresses.use_address(address, selected)?;
            selected.push(Arm64SelectedInstruction::MemoryAddress {
                destination: Arm64SelectedRegister::Fixed(
                    Arm64NocterAbi::allocation_context_register(),
                ),
                source,
            });
            Ok(())
        }
    }
}

fn select_inherited(
    operation: nocter_machine::MachineOperationId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let context = Arm64NocterAbi::allocation_context_register();
    match frame.allocation_context() {
        Arm64AllocationContextFrame::ProgramRoot(object) => {
            selected.push(Arm64SelectedInstruction::MemoryAddress {
                destination: Arm64SelectedRegister::Fixed(context),
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                    object,
                    offset: 0,
                }),
            });
            Ok(())
        }
        Arm64AllocationContextFrame::IncomingPointer(object) => {
            selected.push(Arm64SelectedInstruction::LoadMemory {
                bytes: word_bytes(),
                extension: Arm64SelectedLoadExtension::Zero,
                destination: Arm64SelectedRegister::Fixed(context),
                source: Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject {
                    object,
                    offset: 0,
                }),
            });
            Ok(())
        }
        Arm64AllocationContextFrame::None => Err(Arm64SelectionError::CallAllocation(operation)),
    }
}

/// Reloads the current context before an indirect compiler-generated callback. A callback exists
/// outside the machine function domain, so its need was already propagated into the containing
/// literal function by `MachineAllocationPlan`.
pub(crate) fn select_current(
    operation: nocter_machine::MachineOperationId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    match frame.allocation_context() {
        Arm64AllocationContextFrame::None => Ok(()),
        Arm64AllocationContextFrame::ProgramRoot(_)
        | Arm64AllocationContextFrame::IncomingPointer(_) => {
            select_inherited(operation, frame, selected)
        }
    }
}

/// Materializes the inherited context for a compiler-generated direct call boundary.
pub(crate) fn select_inherited_target(
    program: &nocter_machine::MachineProgram,
    operation: nocter_machine::MachineOperationId,
    target: MachineFunctionId,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let requires_context = program
        .allocation()
        .target_requires_context(&nocter_machine::MachineCallTarget::Direct(target))?;
    if requires_context {
        select_inherited(operation, frame, selected)
    } else {
        Ok(())
    }
}

const fn context_size() -> u64 {
    2 * Arm64NocterAbi::WORD_SIZE
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::WORD_SIZE).expect("the target word size fits selected byte width")
}
