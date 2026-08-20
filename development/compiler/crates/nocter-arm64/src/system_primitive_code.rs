use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64MaterializationError, Arm64NocterAbi,
    Arm64SelectedFunction, Arm64SelectedInstruction, Arm64SelectedRegister,
};

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_EXIT: u64 = 1;

pub(crate) fn emit_selected(
    function: &Arm64SelectedFunction,
    instruction: &Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match *instruction {
        Arm64SelectedInstruction::DarwinSystemCall { argument_count } => {
            emit_system_call(argument_count, code)
        }
        Arm64SelectedInstruction::ExitProcess { status } => emit_exit(function, Some(status), code),
        Arm64SelectedInstruction::Break { immediate } => {
            code.append(Arm64Instruction::Break { immediate });
            Ok(())
        }
        _ => unreachable!("system primitive routing accepts only system instructions"),
    }
}

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
    let syscall_number = crate::frame_access::scratch(0);
    move_register(argument(0), syscall_number, code);
    for position in 0..argument_count {
        move_register(argument(position + 1), argument(position), code);
    }
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });

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
    crate::frame_access::load_immediate(
        code,
        crate::frame_access::scratch(0),
        DARWIN_EXIT,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
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
