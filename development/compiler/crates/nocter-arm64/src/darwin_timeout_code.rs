use crate::{
    Arm64AddSubtract, Arm64CodeBuilder, Arm64DataSize, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi,
};

/// Performs one blocking timeout wait through the target-owned Darwin ABI.
///
/// Source supplies normalized seconds and microseconds in `x0:x1`. This boundary owns the native
/// `timeval`, `select` syscall identity, and zero-or-errno result convention.
pub(crate) fn emit_timeout_wait(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let timeval_size = crate::darwin_kernel_abi::DarwinSelectAbi::TIMEVAL_SIZE;
    crate::frame_access::adjust_stack(code, timeval_size, Arm64AddSubtract::Subtract);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        crate::darwin_kernel_abi::DarwinSelectAbi::SECONDS_OFFSET,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1),
        crate::darwin_kernel_abi::DarwinSelectAbi::MICROSECONDS_OFFSET,
    );
    for index in 0..crate::darwin_kernel_abi::DarwinSelectAbi::TIMEOUT_ARGUMENT_INDEX {
        crate::frame_access::load_immediate(code, argument(index), 0, Arm64DataSize::Bits64);
    }
    crate::frame_access::form_stack_address(
        code,
        argument(crate::darwin_kernel_abi::DarwinSelectAbi::TIMEOUT_ARGUMENT_INDEX),
        0,
    );
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Select,
    );
    crate::system_primitive_code::emit_errno_result(code)?;
    crate::frame_access::adjust_stack(code, timeval_size, Arm64AddSubtract::Add);
    Ok(())
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("Darwin timeout calls use ABI arguments")
}
