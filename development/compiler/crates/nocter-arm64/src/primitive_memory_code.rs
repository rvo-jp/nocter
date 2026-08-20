use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64SelectedFunction, Arm64SelectedRegister,
};

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
