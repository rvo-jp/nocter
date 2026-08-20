use nocter_machine::{MachineOperationId, MachinePrimitiveTarget, MachineResultAbi, PrimitiveRole};

use crate::{
    Arm64NocterAbi, Arm64SelectedInstruction, Arm64SelectedLoadExtension,
    Arm64SelectedMemoryAddress, Arm64SelectedRegister, Arm64SelectionError,
};

/// Expands one closed primitive role while preserving its ordinary Nocter ABI boundary.
pub(crate) fn select(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let offset = match target.role() {
        PrimitiveRole::CurrentAllocatorState => 0,
        PrimitiveRole::CurrentAllocatorKind => Arm64NocterAbi::WORD_SIZE,
        _ => return Err(Arm64SelectionError::PrimitiveCall(operation)),
    };
    validate_context_reader(operation, target)?;
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: word_bytes(),
        extension: Arm64SelectedLoadExtension::Zero,
        destination: Arm64SelectedRegister::Fixed(
            Arm64NocterAbi::argument_register(0)
                .expect("the ABI reserves x0 for a one-word primitive result"),
        ),
        source: Arm64SelectedMemoryAddress::Register {
            base: Arm64SelectedRegister::Fixed(Arm64NocterAbi::allocation_context_register()),
            offset,
        },
    });
    Ok(())
}

fn validate_context_reader(
    operation: MachineOperationId,
    target: &MachinePrimitiveTarget,
) -> Result<(), Arm64SelectionError> {
    if !target.abi().arguments().is_empty() || target.abi().pack().is_some() {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    let MachineResultAbi::Value(result) = target.abi().result() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let nocter_machine::MachineResultLocation::Registers(registers) = result.location() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    if registers.first() != 0 || registers.words() != 1 {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    }
    Ok(())
}

fn word_bytes() -> u8 {
    u8::try_from(Arm64NocterAbi::WORD_SIZE).expect("the target word size fits selected byte width")
}
