use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedRegister,
    Arm64Shift,
};

/// Reads one byte from a validated text view using its explicit length.
pub(crate) fn emit_string_byte(
    code: &mut Arm64CodeBuilder,
    payload_shift: u8,
    absent: u64,
) -> Result<(), Arm64MaterializationError> {
    let argument =
        |index| Arm64NocterAbi::argument_register(index).expect("validated primitive ABI");
    let pointer = argument(0);
    let length = argument(1);
    let index = argument(2);
    let missing = code.create_label();
    let done = code.create_label();
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(index),
        right: Arm64DataRegister::General(length),
    });
    code.branch_conditional(missing, Arm64BranchCondition::CarrySet);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(pointer),
        left: Arm64DataRegister::General(pointer),
        right: Arm64DataRegister::General(index),
    });
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(pointer),
        base: Arm64BaseRegister::General(pointer),
        offset: 0,
    });
    let shift = crate::frame_access::scratch(0);
    crate::frame_access::load_immediate(
        code,
        shift,
        u64::from(payload_shift),
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::VariableShift {
        size: Arm64DataSize::Bits64,
        operation: Arm64Shift::Left,
        destination: pointer,
        value: pointer,
        amount: shift,
    });
    code.branch(done, false);
    code.bind(missing)?;
    crate::frame_access::load_immediate(code, pointer, absent, Arm64DataSize::Bits64);
    code.bind(done)?;
    Ok(())
}

/// Emits the one target-owned runtime loop used by pointer-copy primitives.
///
/// Selection proves that the ranges do not overlap by primitive contract. The loop tests the byte
/// count before touching either pointer, so zero-sized storage may retain its ordinary sentinel.
pub(crate) fn emit_dynamic_copy(
    function: &Arm64SelectedFunction,
    destination: Arm64SelectedRegister,
    source: Arm64SelectedRegister,
    bytes: Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let destination = crate::selected_code::read_register(function, destination, 0, code)?;
    let source = crate::selected_code::read_register(function, source, 1, code)?;
    let bytes = crate::selected_code::read_register(function, bytes, 2, code)?;
    let transfer = crate::frame_access::scratch(0);
    let loop_label = code.create_label();
    let complete = code.create_label();

    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(bytes),
        immediate: 0,
        shift_12: false,
    });
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    code.bind(loop_label)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(transfer),
        base: Arm64BaseRegister::General(source),
        offset: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Byte,
        source: Arm64DataRegister::General(transfer),
        base: Arm64BaseRegister::General(destination),
        offset: 0,
    });
    increment(destination, code);
    increment(source, code);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::General(bytes),
        source: Arm64BaseRegister::General(bytes),
        immediate: 1,
        shift_12: false,
    });
    code.branch_conditional(loop_label, Arm64BranchCondition::NotEqual);
    code.bind(complete)?;
    Ok(())
}

fn increment(register: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register),
        source: Arm64BaseRegister::General(register),
        immediate: 1,
        shift_12: false,
    });
}
