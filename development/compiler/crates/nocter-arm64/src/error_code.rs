use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedInstruction,
    Arm64SelectedRegister,
};

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_WRITE: u64 = 0x0200_0004;
const DARWIN_MUNMAP: u64 = 0x0200_0049;
const DARWIN_MMAP: u64 = 0x0200_00c5;
const STDERR: u64 = 2;
const SEPARATOR_AND_NEWLINE: u64 = u64::from_le_bytes([b':', b' ', b'\n', 0, 0, 0, 0, 0]);

pub(crate) fn emit_selected(
    function: &Arm64SelectedFunction,
    instruction: &Arm64SelectedInstruction,
    allocation_failure_error: crate::Arm64DataId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match *instruction {
        Arm64SelectedInstruction::ReportError { place, buffer } => {
            emit_report(function, place, buffer, code)
        }
        Arm64SelectedInstruction::ReleaseError { place } => emit_release(function, place, code),
        Arm64SelectedInstruction::ConstructErrorLeaf { buffer } => {
            emit_construct_leaf(function, buffer, code)
        }
        Arm64SelectedInstruction::ConstructErrorContext { buffer } => {
            emit_construct_context(function, buffer, code)
        }
        Arm64SelectedInstruction::ReadErrorCode => emit_read_code(code),
        Arm64SelectedInstruction::ReadErrorMessage => {
            emit_read_message(code);
            Ok(())
        }
        Arm64SelectedInstruction::LoadAllocationFailureError => {
            crate::memory_code::emit_data_address(
                function,
                Arm64SelectedRegister::Fixed(argument(0)),
                allocation_failure_error,
                code,
            )
        }
        _ => unreachable!("error materialization accepts only error lifetime instructions"),
    }
}

fn emit_construct_leaf(
    function: &Arm64SelectedFunction,
    buffer: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let schema = Arm64NocterAbi::error();
    let buffer = construction_buffer_offset(function, buffer)?;
    load_stack(code, argument(1), checked_add(buffer, 8)?);
    load_stack(code, scratch(0), checked_add(buffer, 24)?);
    add_register(code, argument(1), argument(1), scratch(0));
    add_immediate(code, argument(1), argument(1), schema.node_payload_offset());
    emit_mmap(code);
    store_stack(code, argument(0), checked_add(buffer, 32)?);
    store_immediate_node_word(
        code,
        argument(0),
        schema.node_kind_offset(),
        schema.owned_leaf_kind(),
    );
    load_stack(code, argument(1), checked_add(buffer, 8)?);
    load_stack(code, scratch(0), checked_add(buffer, 24)?);
    add_register(code, argument(1), argument(1), scratch(0));
    add_immediate(code, argument(1), argument(1), schema.node_payload_offset());
    store_node_word(
        code,
        argument(1),
        argument(0),
        schema.node_allocation_size_offset(),
    );
    store_immediate_node_word(code, argument(0), schema.node_cause_offset(), 0);
    load_stack(code, scratch(0), checked_add(buffer, 8)?);
    store_node_word(
        code,
        scratch(0),
        argument(0),
        schema.node_code_length_offset(),
    );
    load_stack(code, scratch(0), checked_add(buffer, 24)?);
    store_node_word(
        code,
        scratch(0),
        argument(0),
        schema.node_message_length_offset(),
    );

    load_stack(code, argument(2), buffer);
    load_stack(code, argument(3), checked_add(buffer, 8)?);
    add_immediate(code, argument(1), argument(0), schema.node_payload_offset());
    emit_copy_loop(code)?;

    load_stack(code, argument(0), checked_add(buffer, 32)?);
    load_stack(code, scratch(0), checked_add(buffer, 8)?);
    add_immediate(code, argument(1), argument(0), schema.node_payload_offset());
    add_register(code, argument(1), argument(1), scratch(0));
    load_stack(code, argument(2), checked_add(buffer, 16)?);
    load_stack(code, argument(3), checked_add(buffer, 24)?);
    emit_copy_loop(code)?;
    load_stack(code, argument(0), checked_add(buffer, 32)?);
    Ok(())
}

fn emit_construct_context(
    function: &Arm64SelectedFunction,
    buffer: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let schema = Arm64NocterAbi::error();
    let buffer = construction_buffer_offset(function, buffer)?;
    load_stack(code, argument(1), checked_add(buffer, 16)?);
    add_immediate(code, argument(1), argument(1), schema.node_payload_offset());
    emit_mmap(code);
    store_stack(code, argument(0), checked_add(buffer, 32)?);
    store_immediate_node_word(
        code,
        argument(0),
        schema.node_kind_offset(),
        schema.owned_context_kind(),
    );
    load_stack(code, argument(1), checked_add(buffer, 16)?);
    add_immediate(code, argument(1), argument(1), schema.node_payload_offset());
    store_node_word(
        code,
        argument(1),
        argument(0),
        schema.node_allocation_size_offset(),
    );
    load_stack(code, scratch(0), buffer);
    store_node_word(code, scratch(0), argument(0), schema.node_cause_offset());
    store_immediate_node_word(code, argument(0), schema.node_code_length_offset(), 0);
    load_stack(code, scratch(0), checked_add(buffer, 16)?);
    store_node_word(
        code,
        scratch(0),
        argument(0),
        schema.node_message_length_offset(),
    );

    add_immediate(code, argument(1), argument(0), schema.node_payload_offset());
    load_stack(code, argument(2), checked_add(buffer, 8)?);
    load_stack(code, argument(3), checked_add(buffer, 16)?);
    emit_copy_loop(code)?;
    load_stack(code, argument(0), checked_add(buffer, 32)?);
    Ok(())
}

fn emit_mmap(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(2), 3, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(3), 0x1002, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(4), u64::MAX, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(5), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, scratch(0), DARWIN_MMAP, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AllocationFailure.immediate(),
    });
    code.bind(success)
        .expect("the immediately created error allocation label is valid");
}

fn emit_copy_loop(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    let loop_ = code.create_label();
    let complete = code.create_label();
    compare_zero(code, argument(3));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    code.bind(loop_)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Byte,
        destination: Arm64DataRegister::General(scratch(0)),
        base: Arm64BaseRegister::General(argument(2)),
        offset: 0,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Byte,
        source: Arm64DataRegister::General(scratch(0)),
        base: Arm64BaseRegister::General(argument(1)),
        offset: 0,
    });
    add_immediate(code, argument(1), argument(1), 1);
    add_immediate(code, argument(2), argument(2), 1);
    subtract_one(code, argument(3));
    code.branch_conditional(loop_, Arm64BranchCondition::NotEqual);
    code.bind(complete)?;
    Ok(())
}

fn emit_read_code(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    let schema = Arm64NocterAbi::error();
    load_node_word(code, argument(0), argument(0), 0);
    let find_leaf = code.create_label();
    let complete = code.create_label();
    code.bind(find_leaf)?;
    load_node_word(code, scratch(0), argument(0), schema.node_cause_offset());
    compare_zero(code, scratch(0));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    add_immediate(code, argument(0), scratch(0), 0);
    code.branch(find_leaf, false);
    code.bind(complete)?;
    load_node_word(
        code,
        argument(1),
        argument(0),
        schema.node_code_length_offset(),
    );
    add_immediate(code, argument(0), argument(0), schema.node_payload_offset());
    Ok(())
}

fn emit_read_message(code: &mut Arm64CodeBuilder) {
    let schema = Arm64NocterAbi::error();
    load_node_word(code, argument(0), argument(0), 0);
    load_node_word(
        code,
        scratch(0),
        argument(0),
        schema.node_code_length_offset(),
    );
    load_node_word(
        code,
        argument(1),
        argument(0),
        schema.node_message_length_offset(),
    );
    add_immediate(code, argument(0), argument(0), schema.node_payload_offset());
    add_register(code, argument(0), argument(0), scratch(0));
}

fn emit_report(
    function: &Arm64SelectedFunction,
    place: crate::Arm64SelectedMemoryAddress,
    buffer: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let schema = Arm64NocterAbi::error();
    let buffer = object_offset(
        function,
        buffer,
        schema.report_buffer_size(),
        schema.report_buffer_alignment(),
    )?;
    let handle = argument(0);
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(handle),
        place,
        code,
    )?;
    load_node_word(code, handle, handle, 0);
    let node = scratch(0);
    let temporary = scratch(1);
    let separator = checked_add(buffer, schema.report_separator_offset())?;
    let current_node = checked_add(buffer, schema.report_current_node_offset())?;
    let root_handle = checked_add(buffer, schema.report_root_handle_offset())?;

    crate::frame_access::load_immediate(code, node, SEPARATOR_AND_NEWLINE, Arm64DataSize::Bits64);
    store_stack(code, node, separator);
    store_stack(code, handle, current_node);
    store_stack(code, handle, root_handle);

    let find_leaf = code.create_label();
    let leaf_found = code.create_label();
    code.bind(find_leaf)?;
    load_stack(code, node, current_node);
    load_node_word(code, temporary, node, schema.node_cause_offset());
    compare_zero(code, temporary);
    code.branch_conditional(leaf_found, Arm64BranchCondition::Equal);
    store_stack(code, temporary, current_node);
    code.branch(find_leaf, false);
    code.bind(leaf_found)?;

    emit_leaf_code(node, code);
    emit_stack_bytes(separator, 2, code);

    load_stack(code, node, root_handle);
    store_stack(code, node, current_node);
    let report_message = code.create_label();
    let report_complete = code.create_label();
    code.bind(report_message)?;
    load_stack(code, node, current_node);
    load_node_word(code, temporary, node, schema.node_cause_offset());
    store_stack(code, temporary, current_node);
    emit_node_message(node, code);
    load_stack(code, temporary, current_node);
    compare_zero(code, temporary);
    code.branch_conditional(report_complete, Arm64BranchCondition::Equal);
    emit_stack_bytes(separator, 2, code);
    code.branch(report_message, false);
    code.bind(report_complete)?;
    emit_stack_bytes(checked_add(separator, 2)?, 1, code);
    Ok(())
}

fn emit_leaf_code(node: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    let schema = Arm64NocterAbi::error();
    prepare_write(code);
    add_immediate(code, argument(1), node, schema.node_payload_offset());
    load_node_word(code, argument(2), node, schema.node_code_length_offset());
    emit_write(code);
}

fn emit_node_message(node: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    let schema = Arm64NocterAbi::error();
    prepare_write(code);
    add_immediate(code, argument(1), node, schema.node_payload_offset());
    load_node_word(code, scratch(1), node, schema.node_code_length_offset());
    add_register(code, argument(1), argument(1), scratch(1));
    load_node_word(code, argument(2), node, schema.node_message_length_offset());
    emit_write(code);
}

fn emit_release(
    function: &Arm64SelectedFunction,
    place: crate::Arm64SelectedMemoryAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let schema = Arm64NocterAbi::error();
    let head = argument(2);
    crate::memory_code::emit_memory_address(
        function,
        Arm64SelectedRegister::Fixed(head),
        place,
        code,
    )?;
    let node = argument(0);
    let size = argument(1);
    let kind = scratch(0);
    let cause = scratch(1);
    let loop_ = code.create_label();
    let release_owned = code.create_label();
    let advance = code.create_label();
    let complete = code.create_label();

    code.bind(loop_)?;
    load_node_word(code, node, head, 0);
    compare_zero(code, node);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    load_node_word(code, cause, node, schema.node_cause_offset());
    store_node_word(code, cause, head, 0);
    load_node_word(code, kind, node, schema.node_kind_offset());
    compare_immediate(code, kind, schema.static_leaf_kind());
    code.branch_conditional(advance, Arm64BranchCondition::Equal);
    compare_immediate(code, kind, schema.owned_leaf_kind());
    code.branch_conditional(release_owned, Arm64BranchCondition::Equal);
    compare_immediate(code, kind, schema.owned_context_kind());
    code.branch_conditional(release_owned, Arm64BranchCondition::Equal);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::ErrorNodeCorruption.immediate(),
    });

    code.bind(release_owned)?;
    load_node_word(code, size, node, schema.node_allocation_size_offset());
    crate::frame_access::load_immediate(code, kind, DARWIN_MUNMAP, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    code.branch_conditional(advance, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::ErrorReleaseFailure.immediate(),
    });
    code.bind(advance)?;
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn emit_stack_bytes(offset: u64, len: u64, code: &mut Arm64CodeBuilder) {
    prepare_write(code);
    crate::frame_access::form_stack_address(code, argument(1), offset);
    crate::frame_access::load_immediate(code, argument(2), len, Arm64DataSize::Bits64);
    emit_write(code);
}

fn prepare_write(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, argument(0), STDERR, Arm64DataSize::Bits64);
}

fn emit_write(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, scratch(0), DARWIN_WRITE, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
}

fn load_node_word(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    base: crate::Arm64Register,
    offset: u64,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("error node offsets fit an instruction"),
    });
}

fn store_node_word(
    code: &mut Arm64CodeBuilder,
    source: crate::Arm64Register,
    base: crate::Arm64Register,
    offset: u64,
) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("error node offsets fit an instruction"),
    });
}

fn store_immediate_node_word(
    code: &mut Arm64CodeBuilder,
    base: crate::Arm64Register,
    offset: u64,
    value: u64,
) {
    crate::frame_access::load_immediate(code, scratch(0), value, Arm64DataSize::Bits64);
    store_node_word(code, scratch(0), base, offset);
}

fn load_stack(code: &mut Arm64CodeBuilder, destination: crate::Arm64Register, offset: u64) {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        offset,
    );
}

fn store_stack(code: &mut Arm64CodeBuilder, source: crate::Arm64Register, offset: u64) {
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, source, offset);
}

fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    source: crate::Arm64Register,
    immediate: u64,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: u16::try_from(immediate).expect("error header fits an immediate"),
        shift_12: false,
    });
}

fn add_register(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    left: crate::Arm64Register,
    right: crate::Arm64Register,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn subtract_one(code: &mut Arm64CodeBuilder, value: crate::Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::General(value),
        source: Arm64BaseRegister::General(value),
        immediate: 1,
        shift_12: false,
    });
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: crate::Arm64Register) {
    compare_immediate(code, value, 0);
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: crate::Arm64Register, immediate: u64) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate).expect("error node kinds fit an immediate"),
        shift_12: false,
    });
}

fn object_offset(
    function: &Arm64SelectedFunction,
    object: crate::Arm64FrameObjectId,
    size: u64,
    alignment: u64,
) -> Result<u64, Arm64MaterializationError> {
    let object_layout = function
        .frame()
        .layout()
        .object(object)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(object))?;
    if object_layout.size() != size || object_layout.alignment() != alignment {
        return Err(Arm64MaterializationError::InvalidErrorFrame(object));
    }
    Ok(object_layout.offset())
}

fn construction_buffer_offset(
    function: &Arm64SelectedFunction,
    object: crate::Arm64FrameObjectId,
) -> Result<u64, Arm64MaterializationError> {
    object_offset(
        function,
        object,
        5 * Arm64NocterAbi::word_size(),
        Arm64NocterAbi::word_size(),
    )
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64MaterializationError> {
    left.checked_add(right)
        .ok_or(Arm64MaterializationError::OffsetOverflow)
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("error runtime uses ABI argument registers")
}

fn scratch(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves compiler scratch registers")
}
