use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncWaitFrame, Arm64BaseRegister,
    Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction,
    Arm64LoadStoreSize, Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction,
};

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_POLL: u64 = 0x0200_00e6;
const ERRNO_INTERRUPTED: u64 = 4;
const POLL_INPUT: u64 = 1;
const POLL_OUTPUT: u64 = 4;
const POLL_DESCRIPTOR_SIZE: u64 = 8;
const POLL_EVENTS_OFFSET: u32 = 4;
const POLL_RETURNED_EVENTS_OFFSET: u32 = 6;
const MAX_DESCRIPTOR: u64 = i32::MAX as u64;
const MAX_POLL_COUNT: u64 = u32::MAX as u64;

/// Converts one suspended computation's ABI interest slice into a Darwin `poll` wait.
///
/// The process root owns the temporary native descriptor array. The suspended computation owns
/// the interest records and their readiness cells; copied records retain pointers to those cells.
pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let offsets = WaitOffsets::from_function(function)?;
    validate_pending_result(code);
    save_pending_result(offsets, code);
    allocate_poll_descriptors(offsets, code)?;
    translate_interests(offsets, code)?;
    wait_until_ready(offsets, code)?;
    signal_ready_interests(offsets, code)?;
    release_poll_descriptors(offsets, code)?;
    Ok(())
}

fn signal_ready_interests(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_word(offsets.interest_pointer, argument(0), code);
    load_word(offsets.mapping_pointer, argument(1), code);
    load_word(offsets.interest_count, argument(2), code);
    code.append(Arm64Instruction::InstructionSynchronizationBarrier);
    code.append(Arm64Instruction::ReadSystemRegister {
        destination: argument(7),
        register: crate::Arm64SystemRegister::CounterVirtual,
    });

    let scan = code.create_label();
    let descriptor = code.create_label();
    let timer = code.create_label();
    let signal = code.create_label();
    let advance = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load_readiness_pointer(argument(0), argument(5), code)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(3),
        argument(0),
        schema.interest_kind_offset(),
    );
    compare_immediate(argument(3), schema.descriptor_interest_kind(), code);
    code.branch_conditional(descriptor, Arm64BranchCondition::Equal);
    compare_immediate(argument(3), schema.timer_interest_kind(), code);
    code.branch_conditional(timer, Arm64BranchCondition::Equal);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );

    code.bind(descriptor)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Half,
        None,
        argument(4),
        argument(1),
        u64::from(POLL_RETURNED_EVENTS_OFFSET),
    );
    compare_immediate(argument(4), 0, code);
    code.branch_conditional(advance, Arm64BranchCondition::Equal);
    code.branch(signal, false);

    code.bind(timer)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(4),
        argument(0),
        schema.interest_subject_offset(),
    );
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64DataRegister::General(argument(5)),
        left: Arm64DataRegister::General(argument(4)),
        right: Arm64DataRegister::General(argument(7)),
    });
    compare_immediate(argument(5), 0, code);
    code.branch_conditional(signal, Arm64BranchCondition::Equal);
    compare_register_with_immediate(argument(5), i64::MAX as u64, code);
    code.branch_conditional(advance, Arm64BranchCondition::UnsignedLowerOrSame);

    code.bind(signal)?;
    load_readiness_pointer(argument(0), argument(4), code)?;
    crate::frame_access::load_immediate(code, argument(5), 1, Arm64DataSize::Bits64);
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        argument(5),
        argument(4),
        0,
    );

    code.bind(advance)?;
    add_immediate(argument(0), schema.interest_record_size(), code);
    add_immediate(argument(1), POLL_DESCRIPTOR_SIZE, code);
    subtract_immediate(argument(2), 1, code);
    code.branch(scan, false);
    code.bind(complete)
}

fn load_readiness_pointer(
    record: crate::Arm64Register,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        record,
        Arm64NocterAbi::asynchronous().interest_readiness_pointer_offset(),
    );
    compare_immediate(destination, 0, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(valid)
}

#[derive(Clone, Copy)]
struct WaitOffsets {
    interest_pointer: u64,
    interest_count: u64,
    mapping_pointer: u64,
    mapping_size: u64,
    earliest_deadline: u64,
}

impl WaitOffsets {
    fn from_function(function: &Arm64SelectedFunction) -> Result<Self, Arm64MaterializationError> {
        let wait = function
            .frame()
            .async_wait()
            .ok_or(Arm64MaterializationError::MissingAsyncWaitFrame)?;
        let object = function
            .frame()
            .layout()
            .object(wait.object())
            .ok_or(Arm64MaterializationError::UnknownFrameObject(wait.object()))?;
        let field = |offset| {
            object
                .offset()
                .checked_add(offset)
                .ok_or(Arm64MaterializationError::OffsetOverflow)
        };
        Ok(Self {
            interest_pointer: field(Arm64AsyncWaitFrame::INTEREST_POINTER_OFFSET)?,
            interest_count: field(Arm64AsyncWaitFrame::INTEREST_COUNT_OFFSET)?,
            mapping_pointer: field(Arm64AsyncWaitFrame::MAPPING_POINTER_OFFSET)?,
            mapping_size: field(Arm64AsyncWaitFrame::MAPPING_SIZE_OFFSET)?,
            earliest_deadline: field(Arm64AsyncWaitFrame::EARLIEST_DEADLINE_OFFSET)?,
        })
    }
}

fn validate_pending_result(code: &mut Arm64CodeBuilder) {
    let invalid = code.create_label();
    let pointer_present = code.create_label();
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(pointer_present, Arm64BranchCondition::NotEqual);
    code.branch(invalid, false);
    code.bind(pointer_present)
        .expect("fresh async wait label binds once");
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(invalid, Arm64BranchCondition::Equal);
    compare_register_with_immediate(argument(2), MAX_POLL_COUNT, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    code.bind(invalid)
        .expect("fresh async wait label binds once");
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(valid).expect("fresh async wait label binds once");
}

fn save_pending_result(offsets: WaitOffsets, code: &mut Arm64CodeBuilder) {
    store_word(offsets.interest_pointer, argument(1), code);
    store_word(offsets.interest_count, argument(2), code);
    let bytes = argument(3);
    let width = argument(4);
    crate::frame_access::load_immediate(code, width, POLL_DESCRIPTOR_SIZE, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: bytes,
        left: argument(2),
        right: width,
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    store_word(offsets.mapping_size, bytes, code);
}

fn allocate_poll_descriptors(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_word(offsets.mapping_size, argument(1), code);
    crate::darwin_memory_code::emit_map(code)?;
    store_word(offsets.mapping_pointer, argument(0), code);
    Ok(())
}

fn translate_interests(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_word(offsets.interest_pointer, argument(0), code);
    load_word(offsets.mapping_pointer, argument(1), code);
    load_word(offsets.interest_count, argument(2), code);
    crate::frame_access::load_immediate(code, argument(3), u64::MAX, Arm64DataSize::Bits64);

    let scan = code.create_label();
    let descriptor = code.create_label();
    let timer = code.create_label();
    let advance = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(5),
        argument(0),
        schema.interest_readiness_pointer_offset(),
    );
    compare_immediate(argument(5), 0, code);
    let readiness_valid = code.create_label();
    code.branch_conditional(readiness_valid, Arm64BranchCondition::NotEqual);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(readiness_valid)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(4),
        argument(0),
        schema.interest_kind_offset(),
    );
    compare_immediate(argument(4), schema.descriptor_interest_kind(), code);
    code.branch_conditional(descriptor, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), schema.timer_interest_kind(), code);
    code.branch_conditional(timer, Arm64BranchCondition::Equal);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );

    code.bind(descriptor)?;
    emit_descriptor_record(schema, code)?;
    code.branch(advance, false);

    code.bind(timer)?;
    emit_timer_record(schema, code)?;

    code.bind(advance)?;
    add_immediate(argument(0), schema.interest_record_size(), code);
    add_immediate(argument(1), POLL_DESCRIPTOR_SIZE, code);
    subtract_immediate(argument(2), 1, code);
    code.branch(scan, false);

    code.bind(complete)?;
    store_word(offsets.earliest_deadline, argument(3), code);
    Ok(())
}

fn emit_descriptor_record(
    schema: nocter_runtime_contract::RuntimeAsyncAbiSchema,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(5),
        argument(0),
        schema.interest_subject_offset(),
    );
    compare_register_with_immediate(argument(5), MAX_DESCRIPTOR, code);
    let descriptor_valid = code.create_label();
    code.branch_conditional(descriptor_valid, Arm64BranchCondition::UnsignedLowerOrSame);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(descriptor_valid)?;

    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(6),
        argument(0),
        schema.interest_detail_offset(),
    );
    let readable = code.create_label();
    let writable = code.create_label();
    let detail_ready = code.create_label();
    compare_immediate(argument(6), schema.readable_interest_detail(), code);
    code.branch_conditional(readable, Arm64BranchCondition::Equal);
    compare_immediate(argument(6), schema.writable_interest_detail(), code);
    code.branch_conditional(writable, Arm64BranchCondition::Equal);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(readable)?;
    crate::frame_access::load_immediate(code, argument(7), POLL_INPUT, Arm64DataSize::Bits32);
    code.branch(detail_ready, false);
    code.bind(writable)?;
    crate::frame_access::load_immediate(code, argument(7), POLL_OUTPUT, Arm64DataSize::Bits32);
    code.bind(detail_ready)?;
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Word,
        source: Arm64DataRegister::General(argument(5)),
        base: Arm64BaseRegister::General(argument(1)),
        offset: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Half,
        source: Arm64DataRegister::General(argument(7)),
        base: Arm64BaseRegister::General(argument(1)),
        offset: POLL_EVENTS_OFFSET,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Half,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(argument(1)),
        offset: POLL_RETURNED_EVENTS_OFFSET,
    });
    Ok(())
}

fn emit_timer_record(
    schema: nocter_runtime_contract::RuntimeAsyncAbiSchema,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(6),
        argument(0),
        schema.interest_detail_offset(),
    );
    compare_immediate(argument(6), 0, code);
    let detail_valid = code.create_label();
    code.branch_conditional(detail_valid, Arm64BranchCondition::Equal);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
    code.bind(detail_valid)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(5),
        argument(0),
        schema.interest_subject_offset(),
    );
    compare_register(argument(5), argument(3), code);
    let retain = code.create_label();
    code.branch_conditional(retain, Arm64BranchCondition::CarrySet);
    crate::address_code::move_register(code, argument(5), argument(3));
    code.bind(retain)?;
    crate::frame_access::load_immediate(code, argument(7), u32::MAX.into(), Arm64DataSize::Bits32);
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Word,
        source: Arm64DataRegister::General(argument(7)),
        base: Arm64BaseRegister::General(argument(1)),
        offset: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Word,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(argument(1)),
        offset: POLL_EVENTS_OFFSET,
    });
    Ok(())
}

fn wait_until_ready(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let invoke = code.create_label();
    let returned = code.create_label();
    let complete = code.create_label();
    code.bind(invoke)?;
    load_word(offsets.earliest_deadline, argument(3), code);
    crate::async_wait_timeout_code::emit(argument(3), argument(2), code)?;
    load_word(offsets.mapping_pointer, argument(0), code);
    load_word(offsets.interest_count, argument(1), code);
    crate::frame_access::load_immediate(code, scratch(0), DARWIN_POLL, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    code.branch_conditional(returned, Arm64BranchCondition::CarryClear);
    compare_immediate(argument(0), ERRNO_INTERRUPTED, code);
    code.branch_conditional(invoke, Arm64BranchCondition::Equal);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitFailure,
        code,
    );
    code.bind(returned)?;
    load_word(offsets.interest_count, argument(1), code);
    compare_register(argument(0), argument(1), code);
    let count_valid = code.create_label();
    code.branch_conditional(count_valid, Arm64BranchCondition::UnsignedLowerOrSame);
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitFailure,
        code,
    );
    code.bind(count_valid)?;
    compare_immediate(argument(0), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::NotEqual);

    // Darwin's timeout is an i32 millisecond count. A distant absolute deadline is therefore
    // represented by multiple capped polls. A zero-event return resumes the computation only
    // after a fresh monotonic observation proves that the actual deadline has arrived.
    load_word(offsets.earliest_deadline, argument(3), code);
    crate::async_wait_timeout_code::emit(argument(3), argument(2), code)?;
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    code.branch(invoke, false);
    code.bind(complete)
}

fn release_poll_descriptors(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_word(offsets.mapping_pointer, argument(0), code);
    load_word(offsets.mapping_size, argument(1), code);
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitReleaseFailure,
    )?;
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    store_word(offsets.mapping_pointer, argument(0), code);
    Ok(())
}

fn store_word(offset: u64, source: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, source, offset);
}

fn load_word(offset: u64, destination: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        offset,
    );
}

fn compare_immediate(value: crate::Arm64Register, expected: u64, code: &mut Arm64CodeBuilder) {
    if let Ok(immediate) = u16::try_from(expected) {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(value),
            immediate,
            shift_12: false,
        });
    } else {
        compare_register_with_immediate(value, expected, code);
    }
}

fn compare_register_with_immediate(
    value: crate::Arm64Register,
    expected: u64,
    code: &mut Arm64CodeBuilder,
) {
    let temporary = if value == scratch(0) {
        scratch(1)
    } else {
        scratch(0)
    };
    crate::frame_access::load_immediate(code, temporary, expected, Arm64DataSize::Bits64);
    compare_register(value, temporary, code);
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

fn add_immediate(value: crate::Arm64Register, immediate: u64, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("async wait increments fit immediate"),
        shift_12: false,
    });
}

fn subtract_immediate(value: crate::Arm64Register, immediate: u16, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate,
        shift_12: false,
    });
}

fn trap(reason: crate::runtime_trap::Arm64RuntimeTrap, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::Break {
        immediate: reason.immediate(),
    });
}

const fn argument(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::argument_register(index) {
        Some(register) => register,
        None => panic!("async wait uses only ABI argument registers"),
    }
}

const fn scratch(index: u8) -> crate::Arm64Register {
    match Arm64NocterAbi::compiler_scratch_register(index) {
        Some(register) => register,
        None => panic!("async wait uses only reserved scratch registers"),
    }
}
