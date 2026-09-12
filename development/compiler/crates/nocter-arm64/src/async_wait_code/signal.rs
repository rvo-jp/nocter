use super::{
    Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction,
    Arm64LoadStoreSize, Arm64Logical, Arm64NocterAbi, DarwinEventAbiSchema, RuntimeAsyncAbiSchema,
    WaitOffsets, add_immediate, argument, compare_immediate, compare_register,
    compare_register_with_immediate, corrupt, event_list_pointer, load_record, load_record_word,
    load_signed_half_immediate, load_word, require_readiness_pointer, scratch, store_record,
    subtract, subtract_immediate, wait_failure,
};

/// Validates every returned native event against its semantic source before publishing readiness.
pub(super) fn signal_native_events(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    event_list_pointer(offsets, argument(0), code);
    load_word(offsets.event_count, argument(1), code);
    let scan = code.create_label();
    let valid = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load_record(
        argument(0),
        event.flags_offset(),
        Arm64LoadStoreSize::Half,
        argument(2),
        code,
    );
    crate::frame_access::load_immediate(
        code,
        argument(3),
        u64::from(event.error_flag()),
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::LogicalRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64Logical::And,
        destination: Arm64DataRegister::General(argument(2)),
        left: Arm64DataRegister::General(argument(2)),
        right: Arm64DataRegister::General(argument(3)),
    });
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    wait_failure(code);
    code.bind(valid)?;
    validate_and_signal_native_event(offsets, argument(0), code)?;
    add_immediate(argument(0), event.record_size(), code);
    subtract_immediate(argument(1), 1, code);
    code.branch(scan, false);
    code.bind(complete)
}

pub(super) fn validate_and_signal_native_event(
    offsets: WaitOffsets,
    native: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    load_record(
        native,
        event.user_data_offset(),
        Arm64LoadStoreSize::Double,
        argument(4),
        code,
    );
    validate_source_pointer(offsets, argument(4), code)?;
    load_record_word(
        argument(4),
        schema.interest_kind_offset(),
        argument(5),
        code,
    );
    let descriptor = code.create_label();
    let process = code.create_label();
    let signal = code.create_label();
    compare_immediate(argument(5), schema.descriptor_interest_kind(), code);
    code.branch_conditional(descriptor, Arm64BranchCondition::Equal);
    compare_immediate(argument(5), schema.process_exit_interest_kind(), code);
    code.branch_conditional(process, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(descriptor)?;
    validate_descriptor_event(schema, argument(4), native, code)?;
    code.branch(signal, false);
    code.bind(process)?;
    validate_process_event(schema, argument(4), native, code)?;
    code.bind(signal)?;
    require_readiness_pointer(argument(4), argument(5), code)?;
    crate::frame_access::load_immediate(code, argument(6), 1, Arm64DataSize::Bits64);
    store_record(
        argument(5),
        0,
        Arm64LoadStoreSize::Double,
        argument(6),
        code,
    );
    Ok(())
}

fn validate_source_pointer(
    offsets: WaitOffsets,
    source: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let width = Arm64NocterAbi::asynchronous().interest_record_size();
    load_word(offsets.interest_pointer, argument(5), code);
    compare_register(source, argument(5), code);
    let lower_valid = code.create_label();
    code.branch_conditional(lower_valid, Arm64BranchCondition::CarrySet);
    corrupt(code);
    code.bind(lower_valid)?;
    subtract(argument(6), source, argument(5), code);
    load_word(offsets.interest_count, argument(7), code);
    crate::frame_access::load_immediate(code, scratch(0), width, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: argument(7),
        right: scratch(0),
        addend: Arm64DataRegister::Zero,
        subtract_product: false,
    });
    compare_register(argument(6), argument(7), code);
    let upper_valid = code.create_label();
    code.branch_conditional(upper_valid, Arm64BranchCondition::CarryClear);
    corrupt(code);
    code.bind(upper_valid)?;
    code.append(Arm64Instruction::Divide {
        size: Arm64DataSize::Bits64,
        destination: scratch(1),
        left: argument(6),
        right: scratch(0),
        signed: false,
    });
    code.append(Arm64Instruction::MultiplyAdd {
        size: Arm64DataSize::Bits64,
        destination: argument(7),
        left: scratch(1),
        right: scratch(0),
        addend: Arm64DataRegister::General(argument(6)),
        subtract_product: true,
    });
    compare_immediate(argument(7), 0, code);
    let aligned = code.create_label();
    code.branch_conditional(aligned, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(aligned)
}

fn validate_descriptor_event(
    schema: RuntimeAsyncAbiSchema,
    source: crate::Arm64Register,
    native: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    validate_event_subject(schema, source, native, code)?;
    load_record_word(source, schema.interest_detail_offset(), argument(6), code);
    let readable = code.create_label();
    let writable = code.create_label();
    let filter_ready = code.create_label();
    compare_immediate(argument(6), schema.readable_interest_detail(), code);
    code.branch_conditional(readable, Arm64BranchCondition::Equal);
    compare_immediate(argument(6), schema.writable_interest_detail(), code);
    code.branch_conditional(writable, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(readable)?;
    load_signed_half_immediate(argument(7), event.read_filter(), code);
    code.branch(filter_ready, false);
    code.bind(writable)?;
    load_signed_half_immediate(argument(7), event.write_filter(), code);
    code.bind(filter_ready)?;
    validate_event_filter(native, argument(7), code)
}

fn validate_process_event(
    schema: RuntimeAsyncAbiSchema,
    source: crate::Arm64Register,
    native: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    load_record_word(source, schema.interest_detail_offset(), argument(6), code);
    compare_immediate(argument(6), 0, code);
    let detail_valid = code.create_label();
    code.branch_conditional(detail_valid, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(detail_valid)?;
    validate_event_subject(schema, source, native, code)?;
    load_signed_half_immediate(argument(7), event.process_filter(), code);
    validate_event_filter(native, argument(7), code)
}

fn validate_event_subject(
    schema: RuntimeAsyncAbiSchema,
    source: crate::Arm64Register,
    native: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_record_word(source, schema.interest_subject_offset(), argument(6), code);
    load_record(
        native,
        DarwinEventAbiSchema::ARM64_DARWIN.ident_offset(),
        Arm64LoadStoreSize::Double,
        argument(7),
        code,
    );
    compare_register(argument(6), argument(7), code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(valid)
}

fn validate_event_filter(
    native: crate::Arm64Register,
    expected: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_record(
        native,
        DarwinEventAbiSchema::ARM64_DARWIN.filter_offset(),
        Arm64LoadStoreSize::Half,
        argument(6),
        code,
    );
    compare_register(argument(6), expected, code);
    let valid = code.create_label();
    code.branch_conditional(valid, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(valid)
}

/// Publishes every fixed monotonic deadline proven eligible by a fresh counter observation.
pub(super) fn signal_expired_timers(
    offsets: WaitOffsets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_word(offsets.interest_pointer, argument(0), code);
    load_word(offsets.interest_count, argument(1), code);
    code.append(Arm64Instruction::InstructionSynchronizationBarrier);
    code.append(Arm64Instruction::ReadSystemRegister {
        destination: argument(7),
        register: crate::Arm64SystemRegister::CounterVirtual,
    });
    let scan = code.create_label();
    let timer = code.create_label();
    let signal = code.create_label();
    let advance = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(1), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load_record_word(
        argument(0),
        schema.interest_kind_offset(),
        argument(2),
        code,
    );
    compare_immediate(argument(2), schema.timer_interest_kind(), code);
    code.branch_conditional(timer, Arm64BranchCondition::Equal);
    compare_immediate(argument(2), schema.descriptor_interest_kind(), code);
    code.branch_conditional(advance, Arm64BranchCondition::Equal);
    compare_immediate(argument(2), schema.process_exit_interest_kind(), code);
    code.branch_conditional(advance, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(timer)?;
    load_record_word(
        argument(0),
        schema.interest_subject_offset(),
        argument(3),
        code,
    );
    subtract(argument(4), argument(3), argument(7), code);
    compare_immediate(argument(4), 0, code);
    code.branch_conditional(signal, Arm64BranchCondition::Equal);
    compare_register_with_immediate(argument(4), i64::MAX as u64, code);
    code.branch_conditional(advance, Arm64BranchCondition::UnsignedLowerOrSame);
    code.bind(signal)?;
    require_readiness_pointer(argument(0), argument(5), code)?;
    crate::frame_access::load_immediate(code, argument(6), 1, Arm64DataSize::Bits64);
    store_record(
        argument(5),
        0,
        Arm64LoadStoreSize::Double,
        argument(6),
        code,
    );
    code.bind(advance)?;
    add_immediate(argument(0), schema.interest_record_size(), code);
    subtract_immediate(argument(1), 1, code);
    code.branch(scan, false);
    code.bind(complete)
}
