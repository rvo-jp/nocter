use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64Register,
};

/// Observes one wall-clock value through the target-owned Darwin ABI.
///
/// The Nocter result is `(seconds, microseconds, errno)` in caller-owned storage. This boundary
/// owns the native `timeval` layout and syscall identity, and publishes zeroed time fields on
/// failure so source never interprets partially initialized native storage.
pub(crate) fn emit_wall_clock_read(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    const RESULT_POINTER_OFFSET: u64 = crate::darwin_kernel_abi::DarwinTimevalAbi::SIZE;
    const FRAME_SIZE: u64 = RESULT_POINTER_OFFSET + 16;
    crate::frame_access::adjust_stack(code, FRAME_SIZE, Arm64AddSubtract::Subtract);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        Arm64NocterAbi::indirect_result_register(),
        RESULT_POINTER_OFFSET,
    );
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        crate::darwin_kernel_abi::DarwinTimevalAbi::SECONDS_OFFSET,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        crate::darwin_kernel_abi::DarwinTimevalAbi::MICROSECONDS_OFFSET,
    );
    crate::frame_access::form_stack_address(code, argument(0), 0);
    crate::frame_access::load_immediate(code, argument(1), 0, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::GetTimeOfDay,
    );

    let failed = code.create_label();
    let publish = code.create_label();
    code.branch_conditional(failed, Arm64BranchCondition::CarrySet);
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        crate::darwin_kernel_abi::DarwinTimevalAbi::SECONDS_OFFSET,
    );
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1),
        crate::darwin_kernel_abi::DarwinTimevalAbi::MICROSECONDS_OFFSET,
    );
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    code.branch(publish, false);

    code.bind(failed)?;
    move_register(code, argument(2), argument(0));
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(1), 0, Arm64DataSize::Bits64);

    code.bind(publish)?;
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        Arm64NocterAbi::indirect_result_register(),
        RESULT_POINTER_OFFSET,
    );
    for (lane, value) in [argument(0), argument(1), argument(2)]
        .into_iter()
        .enumerate()
    {
        code.append(Arm64Instruction::StoreUnsigned {
            size: Arm64LoadStoreSize::Double,
            source: Arm64DataRegister::General(value),
            base: Arm64BaseRegister::General(Arm64NocterAbi::indirect_result_register()),
            offset: u32::try_from(lane * 8).expect("three-word result offset is bounded"),
        });
    }
    crate::frame_access::adjust_stack(code, FRAME_SIZE, Arm64AddSubtract::Add);
    Ok(())
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: 0,
        shift_12: false,
    });
}

fn argument(index: u8) -> Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("Darwin wall-clock calls use ABI arguments")
}
