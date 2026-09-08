use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister,
};

/// Drives the one computation selected as the process entry and moves its completed output.
pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    computation: Arm64SelectedMemoryAddress,
    destination: Option<Arm64SelectedMemoryAddress>,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let poll = code.create_label();
    let completed = code.create_label();
    let pending = code.create_label();
    code.bind(poll)?;
    load_frame(function, computation, argument(0)?, code)?;
    let resume = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        resume,
        argument(0)?,
        Arm64NocterAbi::asynchronous().resume_function_offset(),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: resume,
        link: true,
    });

    compare_status(
        argument(0)?,
        Arm64NocterAbi::asynchronous().completed_status(),
        code,
    );
    code.branch_conditional(completed, Arm64BranchCondition::Equal);
    compare_status(
        argument(0)?,
        Arm64NocterAbi::asynchronous().pending_status(),
        code,
    );
    code.branch_conditional(pending, Arm64BranchCondition::Equal);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });

    code.bind(pending)?;
    crate::async_wait_code::emit(function, code)?;
    code.branch(poll, false);

    code.bind(completed)?;
    let place = argument(2)?;
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(place),
        computation,
        code,
    )?;
    let frame = argument(0)?;
    crate::address_code::load_native(code, Arm64LoadStoreSize::Double, None, frame, place, 0);
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(place),
        offset: 0,
    });
    match destination {
        Some(destination) => crate::memory_code::emit_memory_address(
            function,
            Arm64SelectedRegister::Fixed(argument(1)?),
            destination,
            code,
        )?,
        None => crate::frame_access::load_immediate(code, argument(1)?, 0, Arm64DataSize::Bits64),
    }
    let consume = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        consume,
        frame,
        Arm64NocterAbi::asynchronous().consume_function_offset(),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: consume,
        link: true,
    });
    Ok(())
}

fn load_frame(
    function: &Arm64SelectedFunction,
    computation: Arm64SelectedMemoryAddress,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let place = argument(2)?;
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(place),
        computation,
        code,
    )?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        place,
        0,
    );
    Ok(())
}

fn compare_status(value: crate::Arm64Register, expected: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(expected).expect("async status tags fit an immediate"),
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
