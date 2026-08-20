use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedInstruction,
};

use crate::process_layout::Arm64ProcessContextLayout;

pub(crate) fn emit_selected(
    function: &Arm64SelectedFunction,
    instruction: &Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match *instruction {
        Arm64SelectedInstruction::InitializeProcessContext { context } => {
            emit_initialize(function, context, code)
        }
        Arm64SelectedInstruction::ReadProcessArgumentCount => {
            load_context_word(
                argument(0),
                Arm64ProcessContextLayout::ARGUMENT_COUNT_OFFSET,
                code,
            );
            Ok(())
        }
        Arm64SelectedInstruction::ReadProcessArgument => {
            emit_indexed_view(ProcessVector::Arguments, code)
        }
        Arm64SelectedInstruction::ReadProcessEnvironmentCount => {
            load_context_word(
                argument(0),
                Arm64ProcessContextLayout::ENVIRONMENT_COUNT_OFFSET,
                code,
            );
            Ok(())
        }
        Arm64SelectedInstruction::ReadProcessEnvironmentName => emit_environment_name(code),
        Arm64SelectedInstruction::ReadProcessEnvironmentValue => emit_environment_value(code),
        _ => unreachable!("process materialization accepts only process instructions"),
    }
}

fn emit_initialize(
    function: &Arm64SelectedFunction,
    context: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let offset = context_offset(function, context)?;
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, Arm64ProcessContextLayout::ARGUMENT_COUNT_OFFSET)?,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1),
        checked_add(offset, Arm64ProcessContextLayout::ARGUMENT_VECTOR_OFFSET)?,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(2),
        checked_add(offset, Arm64ProcessContextLayout::ENVIRONMENT_VECTOR_OFFSET)?,
    );
    emit_count_null_terminated_vector(argument(2), argument(0), code)?;
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, Arm64ProcessContextLayout::ENVIRONMENT_COUNT_OFFSET)?,
    );
    crate::frame_access::form_stack_address(
        code,
        Arm64NocterAbi::process_context_register(),
        offset,
    );
    Ok(())
}

fn emit_count_null_terminated_vector(
    vector: crate::Arm64Register,
    count: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let loop_ = code.create_label();
    let complete = code.create_label();
    let cursor = scratch(0);
    let entry = scratch(1);
    move_register(vector, cursor, code);
    load_immediate(count, 0, code);
    code.bind(loop_)?;
    load_word(entry, cursor, 0, code);
    compare_immediate(entry, 0, Arm64DataSize::Bits64, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_immediate(cursor, cursor, Arm64NocterAbi::WORD_SIZE, code);
    add_immediate(count, count, 1, code);
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum ProcessVector {
    Arguments,
    Environment,
}

impl ProcessVector {
    const fn count_offset(self) -> u64 {
        match self {
            Self::Arguments => Arm64ProcessContextLayout::ARGUMENT_COUNT_OFFSET,
            Self::Environment => Arm64ProcessContextLayout::ENVIRONMENT_COUNT_OFFSET,
        }
    }

    const fn vector_offset(self) -> u64 {
        match self {
            Self::Arguments => Arm64ProcessContextLayout::ARGUMENT_VECTOR_OFFSET,
            Self::Environment => Arm64ProcessContextLayout::ENVIRONMENT_VECTOR_OFFSET,
        }
    }
}

fn emit_indexed_view(
    vector: ProcessVector,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    load_indexed_string(vector, code)?;
    emit_string_length(code)
}

fn load_indexed_string(
    vector: ProcessVector,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let index = argument(0);
    let base = scratch(0);
    let address = scratch(1);
    load_context_word(base, vector.count_offset(), code);
    compare_register(index, base, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
    emit_index_trap(code);
    code.bind(valid)?;
    load_context_word(base, vector.vector_offset(), code);
    code.append(Arm64Instruction::AddSubtractExtendedRegister {
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(address),
        left: Arm64BaseRegister::General(base),
        right: index,
        shift: 3,
    });
    load_word(argument(0), address, 0, code);
    Ok(())
}

fn emit_environment_name(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    load_indexed_string(ProcessVector::Environment, code)?;
    let result = argument(0);
    let length = argument(1);
    let cursor = scratch(0);
    let byte = scratch(1);
    let loop_ = code.create_label();
    let complete = code.create_label();
    move_register(result, cursor, code);
    load_immediate(length, 0, code);
    code.bind(loop_)?;
    load_byte(byte, cursor, code);
    compare_immediate(byte, u64::from(b'='), Arm64DataSize::Bits32, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    compare_immediate(byte, 0, Arm64DataSize::Bits32, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_immediate(cursor, cursor, 1, code);
    add_immediate(length, length, 1, code);
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn emit_environment_value(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    load_indexed_string(ProcessVector::Environment, code)?;
    let result = argument(0);
    let cursor = scratch(0);
    let byte = scratch(1);
    let loop_ = code.create_label();
    let found = code.create_label();
    let missing = code.create_label();
    let measure = code.create_label();
    move_register(result, cursor, code);
    code.bind(loop_)?;
    load_byte(byte, cursor, code);
    compare_immediate(byte, u64::from(b'='), Arm64DataSize::Bits32, code);
    code.branch_conditional(found, Arm64BranchCondition::Equal);
    compare_immediate(byte, 0, Arm64DataSize::Bits32, code);
    code.branch_conditional(missing, Arm64BranchCondition::Equal);
    add_immediate(cursor, cursor, 1, code);
    code.branch(loop_, false);

    code.bind(found)?;
    add_immediate(result, cursor, 1, code);
    code.branch(measure, false);
    code.bind(missing)?;
    move_register(cursor, result, code);
    code.bind(measure)?;
    emit_string_length(code)
}

fn emit_string_length(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    let result = argument(0);
    let length = argument(1);
    let cursor = scratch(0);
    let byte = scratch(1);
    let loop_ = code.create_label();
    let complete = code.create_label();
    move_register(result, cursor, code);
    load_immediate(length, 0, code);
    code.bind(loop_)?;
    load_byte(byte, cursor, code);
    compare_immediate(byte, 0, Arm64DataSize::Bits32, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_immediate(cursor, cursor, 1, code);
    add_immediate(length, length, 1, code);
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn load_context_word(destination: crate::Arm64Register, offset: u64, code: &mut Arm64CodeBuilder) {
    load_word(
        destination,
        Arm64NocterAbi::process_context_register(),
        offset,
        code,
    );
}

fn load_word(
    destination: crate::Arm64Register,
    base: crate::Arm64Register,
    offset: u64,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("process-context word offsets fit the instruction"),
    });
}

fn load_byte(
    destination: crate::Arm64Register,
    base: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: 0,
    });
}

fn compare_register(
    left: crate::Arm64Register,
    right: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn compare_immediate(
    value: crate::Arm64Register,
    immediate: u64,
    size: Arm64DataSize,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("process comparisons use small constants"),
        shift_12: false,
    });
}

fn add_immediate(
    destination: crate::Arm64Register,
    source: crate::Arm64Register,
    immediate: u64,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: u16::try_from(immediate).expect("process increments fit the instruction"),
        shift_12: false,
    });
}

fn move_register(
    source: crate::Arm64Register,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    if source != destination {
        add_immediate(destination, source, 0, code);
    }
}

fn load_immediate(destination: crate::Arm64Register, value: u64, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

fn emit_index_trap(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::ProcessIndexOutOfBounds.immediate(),
    });
}

fn context_offset(
    function: &Arm64SelectedFunction,
    context: crate::Arm64FrameObjectId,
) -> Result<u64, Arm64MaterializationError> {
    let object = function
        .frame()
        .layout()
        .object(context)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(context))?;
    if object.size() != Arm64ProcessContextLayout::SIZE
        || object.alignment() != Arm64ProcessContextLayout::ALIGNMENT
    {
        return Err(Arm64MaterializationError::InvalidProcessContextFrame(
            context,
        ));
    }
    Ok(object.offset())
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64MaterializationError> {
    left.checked_add(right)
        .ok_or(Arm64MaterializationError::OffsetOverflow)
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index)
        .expect("process primitives use only the two-word argument and result window")
}

fn scratch(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves two process scratch registers")
}
