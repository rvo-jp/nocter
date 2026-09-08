use crate::{
    Arm64CodeBuilder, Arm64DataRegister, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister,
};

/// Calls the cancellation entry stored in an owning deferred-computation handle.
pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    place: Arm64SelectedMemoryAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let place_register = argument(2)?;
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(place_register),
        place,
        code,
    )?;
    let frame = argument(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        frame,
        place_register,
        0,
    );
    // Invalidate the consumed owner before crossing the cancellation boundary.
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: crate::Arm64BaseRegister::General(place_register),
        offset: 0,
    });
    let cancel = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        cancel,
        frame,
        Arm64NocterAbi::asynchronous().cancel_function_offset(),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: cancel,
        link: true,
    });
    Ok(())
}

fn argument(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::argument_register(index)
        .ok_or(Arm64MaterializationError::MissingArgumentRegister(index))
}

fn scratch(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::compiler_scratch_register(index)
        .ok_or(Arm64MaterializationError::MissingScratchRegister)
}
