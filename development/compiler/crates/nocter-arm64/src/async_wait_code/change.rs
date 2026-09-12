use super::{
    Arm64BranchCondition, Arm64CodeBuilder, Arm64DataSize, Arm64LoadStoreSize, Arm64NocterAbi,
    DarwinEventAbiSchema, RuntimeAsyncAbiSchema, WaitOffsets, add_immediate, argument,
    compare_immediate, compare_register, corrupt, load_record_word, load_signed_half_immediate,
    load_word, require_native_subject, require_readiness_pointer, store_record, store_record_zero,
    store_word, subtract_immediate,
};

/// Validates semantic records and compacts native interests into one Darwin change list.
pub(super) fn translate_interests(
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
    let process = code.create_label();
    let advance_interest = code.create_label();
    let complete = code.create_label();
    code.bind(scan)?;
    compare_immediate(argument(2), 0, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    require_readiness_pointer(argument(0), argument(5), code)?;
    load_record_word(
        argument(0),
        schema.interest_kind_offset(),
        argument(4),
        code,
    );
    compare_immediate(argument(4), schema.descriptor_interest_kind(), code);
    code.branch_conditional(descriptor, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), schema.timer_interest_kind(), code);
    code.branch_conditional(timer, Arm64BranchCondition::Equal);
    compare_immediate(argument(4), schema.process_exit_interest_kind(), code);
    code.branch_conditional(process, Arm64BranchCondition::Equal);
    corrupt(code);

    code.bind(descriptor)?;
    emit_descriptor_change(schema, argument(0), argument(1), code)?;
    advance_native_change(offsets, argument(1), code);
    code.branch(advance_interest, false);

    code.bind(process)?;
    emit_process_change(schema, argument(0), argument(1), code)?;
    advance_native_change(offsets, argument(1), code);
    code.branch(advance_interest, false);

    code.bind(timer)?;
    update_earliest_deadline(schema, argument(0), argument(3), code)?;

    code.bind(advance_interest)?;
    add_immediate(argument(0), schema.interest_record_size(), code);
    subtract_immediate(argument(2), 1, code);
    code.branch(scan, false);

    code.bind(complete)?;
    store_word(offsets.earliest_deadline, argument(3), code);
    Ok(())
}

fn emit_descriptor_change(
    schema: RuntimeAsyncAbiSchema,
    source: crate::Arm64Register,
    native: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    load_record_word(source, schema.interest_subject_offset(), argument(5), code);
    require_native_subject(argument(5), false, code)?;
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
    write_event_record(
        native,
        source,
        argument(5),
        argument(7),
        u64::from(event.add_flag()),
        0,
        code,
    );
    Ok(())
}

fn emit_process_change(
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
    load_record_word(source, schema.interest_subject_offset(), argument(5), code);
    require_native_subject(argument(5), true, code)?;
    load_signed_half_immediate(argument(7), event.process_filter(), code);
    write_event_record(
        native,
        source,
        argument(5),
        argument(7),
        u64::from(event.add_flag() | event.one_shot_flag()),
        u64::from(event.process_exit_flag()),
        code,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_event_record(
    record: crate::Arm64Register,
    source: crate::Arm64Register,
    subject: crate::Arm64Register,
    filter: crate::Arm64Register,
    flags: u64,
    filter_flags: u64,
    code: &mut Arm64CodeBuilder,
) {
    let event = DarwinEventAbiSchema::ARM64_DARWIN;
    store_record(
        record,
        event.ident_offset(),
        Arm64LoadStoreSize::Double,
        subject,
        code,
    );
    store_record(
        record,
        event.filter_offset(),
        Arm64LoadStoreSize::Half,
        filter,
        code,
    );
    crate::frame_access::load_immediate(code, argument(6), flags, Arm64DataSize::Bits64);
    store_record(
        record,
        event.flags_offset(),
        Arm64LoadStoreSize::Half,
        argument(6),
        code,
    );
    crate::frame_access::load_immediate(code, argument(6), filter_flags, Arm64DataSize::Bits64);
    store_record(
        record,
        event.filter_flags_offset(),
        Arm64LoadStoreSize::Word,
        argument(6),
        code,
    );
    store_record_zero(
        record,
        event.data_offset(),
        Arm64LoadStoreSize::Double,
        code,
    );
    store_record(
        record,
        event.user_data_offset(),
        Arm64LoadStoreSize::Double,
        source,
        code,
    );
    store_record_zero(
        record,
        event.extension_zero_offset(),
        Arm64LoadStoreSize::Double,
        code,
    );
    store_record_zero(
        record,
        event.extension_one_offset(),
        Arm64LoadStoreSize::Double,
        code,
    );
}

fn update_earliest_deadline(
    schema: RuntimeAsyncAbiSchema,
    source: crate::Arm64Register,
    earliest: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), crate::Arm64CodeError> {
    load_record_word(source, schema.interest_detail_offset(), argument(6), code);
    compare_immediate(argument(6), 0, code);
    let detail_valid = code.create_label();
    code.branch_conditional(detail_valid, Arm64BranchCondition::Equal);
    corrupt(code);
    code.bind(detail_valid)?;
    load_record_word(source, schema.interest_subject_offset(), argument(5), code);
    compare_register(argument(5), earliest, code);
    let retain = code.create_label();
    code.branch_conditional(retain, Arm64BranchCondition::CarrySet);
    crate::address_code::move_register(code, argument(5), earliest);
    code.bind(retain)
}

fn advance_native_change(
    offsets: WaitOffsets,
    cursor: crate::Arm64Register,
    code: &mut Arm64CodeBuilder,
) {
    add_immediate(
        cursor,
        DarwinEventAbiSchema::ARM64_DARWIN.record_size(),
        code,
    );
    load_word(offsets.native_count, argument(6), code);
    add_immediate(argument(6), 1, code);
    store_word(offsets.native_count, argument(6), code);
}
