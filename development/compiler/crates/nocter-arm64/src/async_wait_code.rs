use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncWaitFrame, Arm64BaseRegister,
    Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction,
    Arm64LoadStoreSize, Arm64Logical, Arm64MaterializationError, Arm64NocterAbi,
    Arm64SelectedFunction,
};
use nocter_runtime_contract::{DarwinEventAbiSchema, RuntimeAsyncAbiSchema};

use crate::darwin_kernel_abi::{DarwinErrorAbi, DarwinEventQueueAbi};

mod change;
mod signal;

const MILLISECONDS_PER_SECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;

/// Projects one suspended computation's semantic wait set onto one Darwin event queue.
pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let offsets = WaitOffsets::from_function(function)?;
    validate_pending_result(code);
    save_pending_result(offsets, code);
    allocate_event_mapping(offsets, code)?;
    change::translate_interests(offsets, code)?;
    open_event_queue(offsets, code)?;
    submit_native_interests(offsets, code)?;
    wait_until_ready(offsets, code)?;
    close_event_queue(offsets, code)?;
    signal::signal_native_events(offsets, code)?;
    signal::signal_expired_timers(offsets, code)?;
    release_event_mapping(offsets, code)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct WaitOffsets {
    interest_pointer: u64,
    interest_count: u64,
    mapping_pointer: u64,
    mapping_size: u64,
    earliest_deadline: u64,
    native_count: u64,
    event_count: u64,
    queue_descriptor: u64,
    immediate_ready: u64,
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
            native_count: field(Arm64AsyncWaitFrame::NATIVE_COUNT_OFFSET)?,
            event_count: field(Arm64AsyncWaitFrame::EVENT_COUNT_OFFSET)?,
            queue_descriptor: field(Arm64AsyncWaitFrame::QUEUE_DESCRIPTOR_OFFSET)?,
            immediate_ready: field(Arm64AsyncWaitFrame::IMMEDIATE_READY_OFFSET)?,
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
    compare_register_with_immediate(argument(2), DarwinEventQueueAbi::MAX_EVENT_COUNT, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::UnsignedLowerOrSame);
    code.bind(invalid)
        .expect("fresh async wait label binds once");
    corrupt(code);
    code.bind(valid).expect("fresh async wait label binds once");
}

fn save_pending_result(offsets: WaitOffsets, code: &mut Arm64CodeBuilder) {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    store_word(offsets.interest_pointer, argument(1), code);
    store_word(offsets.interest_count, argument(2), code);
    crate::frame_access::load_immediate(
        code,
        argument(3),
        event.record_size() * 2,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(4),
        left: argument(2),
        right: argument(3),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    add_immediate(argument(4), DarwinEventQueueAbi::TIMESPEC_SIZE, code);
    store_word(offsets.mapping_size, argument(4), code);
    crate::frame_access::load_immediate(code, argument(3), 0, Arm64DataSize::Bits64);
    store_word(offsets.native_count, argument(3), code);
    store_word(offsets.event_count, argument(3), code);
    store_word(offsets.immediate_ready, argument(3), code);
}

fn allocate_event_mapping(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_word(offsets.mapping_size, argument(1), code);
    crate::darwin_memory_code::emit_map(code)?;
    store_word(offsets.mapping_pointer, argument(0), code);
    Ok(())
}

fn open_event_queue(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Kqueue,
    );
    let opened = code.create_label();
    code.branch_conditional(opened, Arm64BranchCondition::CarryClear);
    wait_failure(code);
    code.bind(opened)?;
    store_word(offsets.queue_descriptor, argument(0), code);
    Ok(())
}

fn submit_native_interests(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let scan = code.create_label();
    let invoke = code.create_label();
    let missing = code.create_label();
    let advance = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    load_word(offsets.event_count, argument(7), code);
    load_word(offsets.native_count, argument(2), code);
    compare_register(argument(7), argument(2), code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    code.bind(invoke)?;
    load_word(offsets.queue_descriptor, argument(0), code);
    load_word(offsets.event_count, argument(7), code);
    native_record_pointer(offsets, argument(7), argument(1), code);
    crate::frame_access::load_immediate(code, argument(2), 1, Arm64DataSize::Bits64);
    clear_arguments(3, 6, code);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Kevent64,
    );
    let returned = code.create_label();
    code.branch_conditional(returned, Arm64BranchCondition::CarryClear);
    compare_immediate(argument(0), DarwinErrorAbi::INTERRUPTED, code);
    code.branch_conditional(invoke, Arm64BranchCondition::Equal);
    compare_immediate(argument(0), DarwinErrorAbi::MISSING_PROCESS, code);
    code.branch_conditional(missing, Arm64BranchCondition::Equal);
    wait_failure(code);
    code.bind(returned)?;
    compare_immediate(argument(0), 0, code);
    code.branch_conditional(advance, Arm64BranchCondition::Equal);
    wait_failure(code);

    code.bind(missing)?;
    load_word(offsets.event_count, argument(7), code);
    native_record_pointer(offsets, argument(7), argument(1), code);
    load_record(
        argument(1),
        DarwinEventAbiSchema::ARM64_DARWIN.filter_offset(),
        Arm64LoadStoreSize::Half,
        argument(2),
        code,
    );
    load_signed_half_immediate(
        argument(3),
        DarwinEventAbiSchema::ARM64_DARWIN.process_filter(),
        code,
    );
    compare_register(argument(2), argument(3), code);
    let process_missing = code.create_label();
    code.branch_conditional(process_missing, Arm64BranchCondition::Equal);
    wait_failure(code);
    code.bind(process_missing)?;
    signal::validate_and_signal_native_event(offsets, argument(1), code)?;
    crate::frame_access::load_immediate(code, argument(2), 1, Arm64DataSize::Bits64);
    store_word(offsets.immediate_ready, argument(2), code);

    code.bind(advance)?;
    load_word(offsets.event_count, argument(7), code);
    add_immediate(argument(7), 1, code);
    store_word(offsets.event_count, argument(7), code);
    code.branch(scan, false);
    code.bind(complete)?;
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    store_word(offsets.event_count, argument(0), code);
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
    prepare_timeout(offsets, code)?;
    load_word(offsets.queue_descriptor, argument(0), code);
    crate::frame_access::load_immediate(code, argument(1), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    event_list_pointer(offsets, argument(3), code);
    load_word(offsets.interest_count, argument(4), code);
    crate::frame_access::load_immediate(code, argument(5), 0, Arm64DataSize::Bits64);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Kevent64,
    );
    code.branch_conditional(returned, Arm64BranchCondition::CarryClear);
    compare_immediate(argument(0), DarwinErrorAbi::INTERRUPTED, code);
    code.branch_conditional(invoke, Arm64BranchCondition::Equal);
    wait_failure(code);

    code.bind(returned)?;
    load_word(offsets.native_count, argument(1), code);
    compare_register(argument(0), argument(1), code);
    let count_valid = code.create_label();
    code.branch_conditional(count_valid, Arm64BranchCondition::UnsignedLowerOrSame);
    wait_failure(code);
    code.bind(count_valid)?;
    store_word(offsets.event_count, argument(0), code);
    compare_immediate(argument(0), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::NotEqual);
    load_word(offsets.immediate_ready, argument(1), code);
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::NotEqual);
    load_word(offsets.earliest_deadline, argument(3), code);
    crate::async_wait_timeout_code::emit(argument(3), argument(2), code)?;
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    code.branch(invoke, false);
    code.bind(complete)
}

fn prepare_timeout(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let immediate = code.create_label();
    let milliseconds_ready = code.create_label();
    load_word(offsets.immediate_ready, argument(0), code);
    compare_immediate(argument(0), 0, code);
    code.branch_conditional(immediate, Arm64BranchCondition::NotEqual);
    load_word(offsets.earliest_deadline, argument(3), code);
    crate::async_wait_timeout_code::emit(argument(3), argument(2), code)?;
    code.branch(milliseconds_ready, false);
    code.bind(immediate)?;
    crate::frame_access::load_immediate(code, argument(2), 0, Arm64DataSize::Bits64);
    code.bind(milliseconds_ready)?;
    let infinite = code.create_label();
    let complete = code.create_label();
    compare_immediate(argument(2), u64::MAX, code);
    code.branch_conditional(infinite, Arm64BranchCondition::Equal);
    timeout_pointer(offsets, argument(6), code);
    crate::frame_access::load_immediate(
        code,
        argument(4),
        MILLISECONDS_PER_SECOND,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::Divide {
        size: Arm64DataSize::Bits64,
        destination: argument(5),
        left: argument(2),
        right: argument(4),
        signed: false,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: argument(5),
        right: argument(4),
        addend: Arm64DataRegister::General(argument(2)),
        subtract_product: true,
    });
    crate::frame_access::load_immediate(
        code,
        argument(4),
        NANOSECONDS_PER_MILLISECOND,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: argument(7),
        right: argument(4),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    store_record(
        argument(6),
        DarwinEventQueueAbi::TIMESPEC_SECONDS_OFFSET,
        Arm64LoadStoreSize::Double,
        argument(5),
        code,
    );
    store_record(
        argument(6),
        DarwinEventQueueAbi::TIMESPEC_NANOSECONDS_OFFSET,
        Arm64LoadStoreSize::Double,
        argument(7),
        code,
    );
    code.branch(complete, false);
    code.bind(infinite)?;
    crate::frame_access::load_immediate(code, argument(6), 0, Arm64DataSize::Bits64);
    code.bind(complete)
}

fn close_event_queue(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_word(offsets.queue_descriptor, argument(0), code);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Close,
    );
    let closed = code.create_label();
    code.branch_conditional(closed, Arm64BranchCondition::CarryClear);
    wait_failure(code);
    code.bind(closed)
}

fn release_event_mapping(
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

fn require_native_subject(
    subject: crate::Arm64Register,
    positive: bool,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    if positive {
        compare_immediate(subject, 0, code);
        let positive_label = code.create_label();
        code.branch_conditional(positive_label, Arm64BranchCondition::NotEqual);
        corrupt(code);
        code.bind(positive_label)?;
    }
    compare_register_with_immediate(subject, DarwinEventQueueAbi::MAX_SUBJECT, code);
    let bounded = code.create_label();
    code.branch_conditional(bounded, Arm64BranchCondition::UnsignedLowerOrSame);
    corrupt(code);
    code.bind(bounded)
}

fn require_readiness_pointer(
    record: crate::Arm64Register,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_record_word(
        record,
        Arm64NocterAbi::asynchronous().interest_readiness_pointer_offset(),
        destination,
        code,
    );
    compare_immediate(destination, 0, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::NotEqual);
    corrupt(code);
    code.bind(valid)
}

fn event_list_pointer(
    offsets: WaitOffsets,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    load_word(offsets.mapping_pointer, destination, code);
    add_scaled_interest_count(
        offsets,
        destination,
        DarwinEventAbiSchema::ARM64_DARWIN.record_size(),
        code,
    );
}

fn timeout_pointer(
    offsets: WaitOffsets,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    load_word(offsets.mapping_pointer, destination, code);
    add_scaled_interest_count(
        offsets,
        destination,
        DarwinEventAbiSchema::ARM64_DARWIN.record_size() * 2,
        code,
    );
}

fn add_scaled_interest_count(
    offsets: WaitOffsets,
    destination: crate::Arm64Register,
    scale: u64,
    code: &mut Arm64CodeBuilder,
) {
    load_word(offsets.interest_count, argument(7), code);
    crate::frame_access::load_immediate(code, scratch(0), scale, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination,
        left: argument(7),
        right: scratch(0),
        addend: Arm64DataRegister::General(destination),
        subtract_product: false,
    });
}

fn native_record_pointer(
    offsets: WaitOffsets,
    index: crate::Arm64Register,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    load_word(offsets.mapping_pointer, destination, code);
    crate::frame_access::load_immediate(
        code,
        scratch(0),
        DarwinEventAbiSchema::ARM64_DARWIN.record_size(),
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination,
        left: index,
        right: scratch(0),
        addend: Arm64DataRegister::General(destination),
        subtract_product: false,
    });
}

fn clear_arguments(first: u8, last: u8, code: &mut Arm64CodeBuilder) {
    for index in first..=last {
        crate::frame_access::load_immediate(code, argument(index), 0, Arm64DataSize::Bits64);
    }
}

fn load_signed_half_immediate(
    destination: crate::Arm64Register,
    value: i16,
    code: &mut Arm64CodeBuilder,
) {
    crate::frame_access::load_immediate(
        code,
        destination,
        u64::from(value.cast_unsigned()),
        Arm64DataSize::Bits64,
    );
}

fn load_record_word(
    record: crate::Arm64Register,
    offset: u64,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    load_record(
        record,
        offset,
        Arm64LoadStoreSize::Double,
        destination,
        code,
    );
}

fn load_record(
    record: crate::Arm64Register,
    offset: u64,
    size: Arm64LoadStoreSize,
    destination: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::load_native(code, size, None, destination, record, offset);
}

fn store_record(
    record: crate::Arm64Register,
    offset: u64,
    size: Arm64LoadStoreSize,
    source: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    crate::address_code::store_native(code, size, source, record, offset);
}

fn store_record_zero(
    record: crate::Arm64Register,
    offset: u64,
    size: Arm64LoadStoreSize,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::General(record),
        offset: u32::try_from(offset).expect("Darwin event record offsets fit immediate storage"),
    });
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

fn subtract(
    destination: crate::Arm64Register,
    left: crate::Arm64Register,
    right: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn corrupt(code: &mut Arm64CodeBuilder) {
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitRecordCorruption,
        code,
    );
}

fn wait_failure(code: &mut Arm64CodeBuilder) {
    trap(
        crate::runtime_trap::Arm64RuntimeTrap::AsyncWaitFailure,
        code,
    );
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
