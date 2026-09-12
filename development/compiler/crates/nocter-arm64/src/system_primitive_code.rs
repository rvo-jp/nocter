use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64MaterializationError, Arm64NocterAbi,
    Arm64SelectedFunction, Arm64SelectedRegister,
};

/// Translates the ordinary Nocter primitive ABI into Darwin's syscall register convention.
pub(crate) fn emit_system_call(
    argument_count: u8,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    if argument_count > 6 {
        return Err(Arm64MaterializationError::InvalidSystemCallArity(
            argument_count,
        ));
    }
    let syscall_number = crate::darwin_kernel_abi::system_call_register();
    move_register(argument(0), syscall_number, code);
    for position in 0..argument_count {
        move_register(argument(position + 1), argument(position), code);
    }
    crate::darwin_kernel_abi::emit_loaded_system_call(code);
    emit_system_call_result(code)
}

/// Translates Darwin's carry-set error convention into Nocter's `(value, errno)` result.
pub(crate) fn emit_system_call_result(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let success = code.create_label();
    let complete = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    move_register(argument(0), argument(1), code);
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    code.branch(complete, false);
    code.bind(success)?;
    crate::frame_access::load_immediate(code, argument(1), 0, Arm64DataSize::Bits64);
    code.bind(complete)?;
    Ok(())
}

/// Closes the descriptor in `x0` through the compiler-owned Darwin ABI identity.
pub(crate) fn emit_descriptor_close(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Close,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_descriptor_duplicate_close_on_exec(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::DUPLICATE_CLOSE_ON_EXEC,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(
        code,
        argument(2),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::FIRST_PRIVATE_DESCRIPTOR,
        Arm64DataSize::Bits64,
    );
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Fcntl,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_descriptor_status_flags(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::GET_STATUS_FLAGS,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Fcntl,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_descriptor_set_status_flags(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    move_register(argument(1), argument(2), code);
    crate::frame_access::load_immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::SET_STATUS_FLAGS,
        Arm64DataSize::Bits64,
    );
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Fcntl,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_descriptor_suppress_broken_pipe(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDescriptorAbi::SET_NO_SIGPIPE,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(code, argument(2), 1, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Fcntl,
    );
    emit_system_call_result(code)
}

/// Reads once through the closed nonblocking-descriptor contract in `x0..x2`.
pub(crate) fn emit_descriptor_read(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Read,
    );
    emit_system_call_result(code)
}

/// Writes once through the closed nonblocking-descriptor contract in `x0..x2`.
pub(crate) fn emit_descriptor_write(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Write,
    );
    emit_system_call_result(code)
}

/// Calls `wait4(pid, status, WNOHANG, NULL)` and returns `(value, errno)`.
pub(crate) fn emit_process_observe(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(
        code,
        argument(2),
        crate::darwin_kernel_abi::DarwinProcessAbi::OBSERVE_WITHOUT_WAITING,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(code, argument(3), 0, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Wait4,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_process_open_null(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Open,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_process_install_descriptor(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Dup2,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_process_change_directory(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Chdir,
    );
    emit_system_call_result(code)
}

pub(crate) fn emit_process_exec(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Execve,
    );
    emit_system_call_result(code)
}

/// Fills the `u64` addressed by `x0` and returns zero or the Darwin errno in `x0`.
pub(crate) fn emit_entropy_seed_fill(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::frame_access::load_immediate(code, argument(1), 8, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::GetEntropy,
    );
    emit_errno_result(code)
}

/// Translates Darwin's carry convention into a single zero-or-errno result in `x0`.
pub(crate) fn emit_errno_result(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let complete = code.create_label();
    code.branch_conditional(complete, Arm64BranchCondition::CarrySet);
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    code.bind(complete)?;
    Ok(())
}

pub(crate) fn emit_fixed_system_call_pair(
    call: crate::darwin_kernel_abi::DarwinSystemCall,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    crate::darwin_kernel_abi::emit_system_call(code, call);
    emit_system_call_pair_result(code)
}

fn emit_system_call_pair_result(
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let success = code.create_label();
    let complete = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    move_register(argument(0), argument(2), code);
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(1), 0, Arm64DataSize::Bits64);
    code.branch(complete, false);
    code.bind(success)?;
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    code.bind(complete)?;
    Ok(())
}

pub(crate) fn emit_exit(
    function: &Arm64SelectedFunction,
    status: Option<Arm64SelectedRegister>,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let status_register = argument(0);
    if let Some(status) = status {
        let source = crate::selected_code::read_register(function, status, 0, code)?;
        move_register(source, status_register, code);
    } else {
        crate::frame_access::load_immediate(code, status_register, 0, Arm64DataSize::Bits64);
    }
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Exit,
    );
    Ok(())
}

fn move_register(
    source: crate::Arm64Register,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    if source == destination {
        return;
    }
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

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index)
        .expect("the validated syscall arity fits the Nocter argument-register window")
}
