use crate::{
    Arm64BaseRegister, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction, Arm64LoadStoreSize, Arm64MaterializationError, Arm64NocterAbi,
    Arm64SelectedFunction, Arm64SelectedMemoryAddress, Arm64SelectedRegister,
};

pub(crate) fn emit_release(
    function: &Arm64SelectedFunction,
    place: Arm64SelectedMemoryAddress,
    staging: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let staging_offset = release_staging_offset(function, staging)?;
    let place_register = argument(2)?;
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(place_register),
        place,
        code,
    )?;
    let environment = argument(0)?;
    let mapped_size = argument(1)?;
    load_word(code, environment, place_register, 0);
    load_word(code, mapped_size, place_register, 24);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        environment,
        staging_offset,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        mapped_size,
        staging_offset + Arm64NocterAbi::word_size(),
    );
    // Invalidate the owner before crossing a user-authored destruction boundary.
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(place_register),
        offset: 0,
    });
    let destroy = scratch(0)?;
    load_word(code, destroy, place_register, 16);
    let skip_destroy = code.create_label();
    compare_zero(code, destroy);
    code.branch_conditional(skip_destroy, Arm64BranchCondition::Equal);
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0)?,
        staging_offset,
    );
    crate::frame_access::load_immediate(code, argument(1)?, 0, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::BranchRegister {
        target: destroy,
        link: true,
    });
    code.bind(skip_destroy)?;

    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0)?,
        staging_offset,
    );
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1)?,
        staging_offset + Arm64NocterAbi::word_size(),
    );
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::ErasedCallableReleaseFailure,
    )?;
    Ok(())
}

fn release_staging_offset(
    function: &Arm64SelectedFunction,
    staging: crate::Arm64FrameObjectId,
) -> Result<u64, Arm64MaterializationError> {
    let object = function
        .frame()
        .layout()
        .object(staging)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(staging))?;
    if object.size() != 2 * Arm64NocterAbi::word_size()
        || object.alignment() != Arm64NocterAbi::word_size()
    {
        return Err(Arm64MaterializationError::InvalidErasedCallableReleaseFrame(staging));
    }
    Ok(object.offset())
}

fn load_word(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    base: crate::Arm64Register,
    offset: u64,
) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        base,
        offset,
    );
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: crate::Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: crate::Arm64AddSubtractDestination::Zero,
        source: crate::Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn argument(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::argument_register(index)
        .ok_or(Arm64MaterializationError::MissingArgumentRegister(index))
}

fn scratch(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::compiler_scratch_register(index)
        .ok_or(Arm64MaterializationError::MissingScratchRegister)
}
