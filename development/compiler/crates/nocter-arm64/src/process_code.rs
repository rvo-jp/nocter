use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LabelId,
    Arm64LoadStoreSize, Arm64MaterializationError, Arm64NocterAbi, Arm64ProcessTextResult,
    Arm64ProcessTextSource, Arm64SelectedFunction,
};

const ENVIRONMENT_SEPARATOR: u8 = b'=';

pub(crate) fn emit_initialize(
    function: &Arm64SelectedFunction,
    context: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let layout = Arm64NocterAbi::process_context();
    let offset = context_offset(function, context)?;
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, layout.argument_count_offset())?,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1),
        checked_add(offset, layout.argument_vector_offset())?,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(2),
        checked_add(offset, layout.environment_vector_offset())?,
    );
    emit_count_null_terminated_vector(argument(2), argument(0), code)?;
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, layout.environment_count_offset())?,
    );
    load_immediate(argument(0), 0, code);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, layout.blocking_service_pointer_offset())?,
    );
    load_immediate(argument(0), u64::MAX, code);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(0),
        checked_add(offset, layout.lifecycle_descriptor_offset())?,
    );
    load_immediate(argument(0), 0, code);
    for field in [
        layout.lifecycle_reload_handler_offset(),
        layout.lifecycle_interrupt_handler_offset(),
        layout.lifecycle_terminate_handler_offset(),
        layout.lifecycle_sticky_termination_offset(),
    ] {
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            argument(0),
            checked_add(offset, field)?,
        );
    }
    crate::frame_access::form_stack_address(
        code,
        Arm64NocterAbi::process_context_register(),
        offset,
    );
    Ok(())
}

pub(crate) fn emit_argument_count(code: &mut Arm64CodeBuilder) {
    load_context_word(
        argument(0),
        Arm64NocterAbi::process_context().argument_count_offset(),
        code,
    );
}

pub(crate) fn emit_text(
    source: Arm64ProcessTextSource,
    result: Arm64ProcessTextResult,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match result {
        Arm64ProcessTextResult::RawView => {
            compare_index(source.vector(), code);
            let valid = code.create_label();
            code.branch_conditional(valid, Arm64BranchCondition::CarryClear);
            emit_index_trap(code);
            code.bind(valid)?;
            select_text_pointer(source, code)?;
            emit_string_length(code, source.name_boundary())
        }
        Arm64ProcessTextResult::Checked {
            entry_tag,
            encoding_tag,
            pointer,
            length,
        } => emit_checked_text(source, code, entry_tag, encoding_tag, pointer, length),
    }
}

fn emit_checked_text(
    source: Arm64ProcessTextSource,
    code: &mut Arm64CodeBuilder,
    entry_tag: u64,
    encoding_tag: u64,
    pointer: u64,
    length: u64,
) -> Result<(), Arm64MaterializationError> {
    let missing = code.create_label();
    let invalid = code.create_label();
    let done = code.create_label();
    compare_index(source.vector(), code);
    code.branch_conditional(missing, Arm64BranchCondition::CarrySet);
    select_text_pointer(source, code)?;
    store_optional_tag(code, entry_tag, true)?;
    emit_checked_string_length(code, invalid, source.name_boundary())?;
    store_optional_tag(code, encoding_tag, true)?;
    store_result(code, Arm64LoadStoreSize::Double, argument(0), pointer)?;
    store_result(code, Arm64LoadStoreSize::Double, argument(1), length)?;
    code.branch(done, false);

    code.bind(invalid)?;
    store_optional_tag(code, encoding_tag, false)?;
    code.branch(done, false);

    code.bind(missing)?;
    store_optional_tag(code, entry_tag, false)?;
    code.bind(done)?;
    Ok(())
}

fn store_optional_tag(
    code: &mut Arm64CodeBuilder,
    offset: u64,
    present: bool,
) -> Result<(), Arm64MaterializationError> {
    let optional = nocter_machine::MachineOutcomeKind::Optional;
    let tag = if present {
        optional.primary_tag()
    } else {
        optional.alternate_tag()
    };
    load_immediate(scratch(1), u64::from(tag), code);
    store_result(code, Arm64LoadStoreSize::Byte, scratch(1), offset)
}

fn store_result(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: crate::Arm64Register,
    offset: u64,
) -> Result<(), Arm64MaterializationError> {
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(Arm64NocterAbi::indirect_result_register()),
        offset: u32::try_from(offset).map_err(|_| Arm64MaterializationError::OffsetOverflow)?,
    });
    Ok(())
}

pub(crate) fn emit_environment_count(code: &mut Arm64CodeBuilder) {
    load_context_word(
        argument(0),
        Arm64NocterAbi::process_context().environment_count_offset(),
        code,
    );
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
    add_immediate(cursor, cursor, Arm64NocterAbi::word_size(), code);
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
        let layout = Arm64NocterAbi::process_context();
        match self {
            Self::Arguments => layout.argument_count_offset(),
            Self::Environment => layout.environment_count_offset(),
        }
    }

    const fn vector_offset(self) -> u64 {
        let layout = Arm64NocterAbi::process_context();
        match self {
            Self::Arguments => layout.argument_vector_offset(),
            Self::Environment => layout.environment_vector_offset(),
        }
    }
}

impl Arm64ProcessTextSource {
    const fn vector(self) -> ProcessVector {
        match self {
            Self::Argument => ProcessVector::Arguments,
            Self::EnvironmentName | Self::EnvironmentValue => ProcessVector::Environment,
        }
    }

    const fn name_boundary(self) -> bool {
        matches!(self, Self::EnvironmentName)
    }
}

fn select_text_pointer(
    source: Arm64ProcessTextSource,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    load_string_pointer(source.vector(), code);
    if source == Arm64ProcessTextSource::EnvironmentValue {
        skip_environment_name(code)?;
    }
    Ok(())
}

fn compare_index(vector: ProcessVector, code: &mut Arm64CodeBuilder) {
    load_context_word(scratch(0), vector.count_offset(), code);
    compare_register(argument(0), scratch(0), code);
}

fn load_string_pointer(vector: ProcessVector, code: &mut Arm64CodeBuilder) {
    let index = argument(0);
    let base = scratch(0);
    let address = scratch(1);
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
}

fn skip_environment_name(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    let cursor = argument(0);
    let byte = scratch(1);
    let loop_ = code.create_label();
    let found = code.create_label();
    let complete = code.create_label();
    code.bind(loop_)?;
    load_byte(byte, cursor, code);
    compare_immediate(
        byte,
        u64::from(ENVIRONMENT_SEPARATOR),
        Arm64DataSize::Bits32,
        code,
    );
    code.branch_conditional(found, Arm64BranchCondition::Equal);
    compare_immediate(byte, 0, Arm64DataSize::Bits32, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_immediate(cursor, cursor, 1, code);
    code.branch(loop_, false);

    code.bind(found)?;
    add_immediate(cursor, cursor, 1, code);
    code.bind(complete)?;
    Ok(())
}

fn emit_string_length(
    code: &mut Arm64CodeBuilder,
    name_boundary: bool,
) -> Result<(), Arm64MaterializationError> {
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
    if name_boundary {
        compare_immediate(
            byte,
            u64::from(ENVIRONMENT_SEPARATOR),
            Arm64DataSize::Bits32,
            code,
        );
        code.branch_conditional(complete, Arm64BranchCondition::Equal);
    }
    add_immediate(cursor, cursor, 1, code);
    add_immediate(length, length, 1, code);
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn emit_checked_string_length(
    code: &mut Arm64CodeBuilder,
    invalid: Arm64LabelId,
    name_boundary: bool,
) -> Result<(), Arm64MaterializationError> {
    use Arm64BranchCondition::{CarryClear, CarrySet, Equal, UnsignedHigher};
    let cursor = scratch(0);
    let byte = scratch(1);
    let remaining = argument(2);
    let minimum = argument(3);
    let maximum = argument(4);
    let head = code.create_label();
    let complete = code.create_label();
    let two = code.create_label();
    let three = code.create_label();
    let default_tail = code.create_label();
    let tail = code.create_label();
    let supplementary_start = code.create_label();
    let unicode_limit = code.create_label();
    let exclude_overlong = code.create_label();
    let exclude_surrogate = code.create_label();
    let compare = |code: &mut Arm64CodeBuilder, immediate, target, condition| {
        compare_immediate(byte, immediate, Arm64DataSize::Bits32, code);
        code.branch_conditional(target, condition);
    };

    move_register(argument(0), cursor, code);
    code.bind(head)?;
    load_byte(byte, cursor, code);
    compare(code, 0, complete, Equal);
    if name_boundary {
        compare(code, u64::from(ENVIRONMENT_SEPARATOR), complete, Equal);
    }
    add_immediate(cursor, cursor, 1, code);
    compare(code, 0x80, head, CarryClear);
    compare(code, 0xC2, invalid, CarryClear);
    compare(code, 0xE0, two, CarryClear);
    compare(code, 0xF0, three, CarryClear);
    compare(code, 0xF5, invalid, CarrySet);

    // Four-byte scalar. Restrict the first continuation at both Unicode boundaries.
    load_immediate(remaining, 3, code);
    compare(code, 0xF0, supplementary_start, Equal);
    compare(code, 0xF4, unicode_limit, Equal);
    code.branch(default_tail, false);

    code.bind(two)?;
    load_immediate(remaining, 1, code);
    code.branch(default_tail, false);

    code.bind(three)?;
    load_immediate(remaining, 2, code);
    compare(code, 0xE0, exclude_overlong, Equal);
    compare(code, 0xED, exclude_surrogate, Equal);
    code.branch(default_tail, false);

    for (label, low, high) in [
        (supplementary_start, 0x90, 0xBF),
        (unicode_limit, 0x80, 0x8F),
        (exclude_overlong, 0xA0, 0xBF),
        (exclude_surrogate, 0x80, 0x9F),
    ] {
        code.bind(label)?;
        load_immediate(minimum, low, code);
        load_immediate(maximum, high, code);
        code.branch(tail, false);
    }

    code.bind(default_tail)?;
    load_immediate(minimum, 0x80, code);
    load_immediate(maximum, 0xBF, code);
    code.bind(tail)?;
    load_byte(byte, cursor, code);
    compare_register(byte, minimum, code);
    code.branch_conditional(invalid, CarryClear);
    compare_register(byte, maximum, code);
    code.branch_conditional(invalid, UnsignedHigher);
    add_immediate(cursor, cursor, 1, code);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::General(remaining),
        source: Arm64BaseRegister::General(remaining),
        immediate: 1,
        shift_12: false,
    });
    code.branch_conditional(head, Equal);
    code.branch(default_tail, false);

    code.bind(complete)?;
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64DataRegister::General(argument(1)),
        left: Arm64DataRegister::General(cursor),
        right: Arm64DataRegister::General(argument(0)),
    });
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
    let layout = Arm64NocterAbi::process_context();
    let object = function
        .frame()
        .layout()
        .object(context)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(context))?;
    if object.size() != layout.size() || object.alignment() != layout.alignment() {
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
        .expect("process primitives use caller-saved argument registers")
}

fn scratch(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves two process scratch registers")
}
