use crate::{
    Arm64BranchCondition, Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64PackDescriptorLayout, Arm64SelectedFunction,
    Arm64SelectedLoadExtension, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
};

/// Releases a pack descriptor only when it owns allocation-backed deferred state.
pub(crate) fn emit_release_selected(
    function: &Arm64SelectedFunction,
    source: Arm64SelectedMemoryAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let descriptor = argument(2)?;
    crate::memory_code::emit_memory_load(
        function,
        8,
        Arm64SelectedLoadExtension::Zero,
        Arm64SelectedRegister::Fixed(descriptor),
        source,
        code,
    )?;
    emit_release(descriptor, code)
}

pub(crate) fn emit_release(
    descriptor: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let allocation_size = argument_avoiding(1, descriptor)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        allocation_size,
        descriptor,
        Arm64PackDescriptorLayout::ALLOCATION_SIZE_OFFSET,
    );
    let complete = code.create_label();
    compare_zero(allocation_size, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    crate::address_code::move_register(code, descriptor, argument(0)?);
    if allocation_size != argument(1)? {
        crate::address_code::move_register(code, allocation_size, argument(1)?);
    }
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    code.bind(complete)?;
    Ok(())
}

fn compare_zero(register: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: crate::Arm64AddSubtractDestination::Zero,
        source: crate::Arm64BaseRegister::General(register),
        immediate: 0,
        shift_12: false,
    });
}

fn argument(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::argument_register(index)
        .ok_or(Arm64MaterializationError::MissingArgumentRegister(index))
}

fn argument_avoiding(
    index: u8,
    occupied: crate::Arm64Register,
) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    let preferred = argument(index)?;
    if preferred != occupied {
        return Ok(preferred);
    }
    Arm64NocterAbi::compiler_scratch_register(0)
        .filter(|candidate| *candidate != occupied)
        .ok_or(Arm64MaterializationError::MissingScratchRegister)
}
