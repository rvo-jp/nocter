use crate::async_activation::{
    Arm64AsyncActivationField, Arm64AsyncActivationState, Arm64AsyncActivationTarget,
};
use crate::{
    Arm64AddSubtract, Arm64AsyncFrameField, Arm64AsyncFunctionPlan, Arm64AsyncResumeError,
    Arm64BranchCondition, Arm64Code, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64FrameCode, Arm64FrameObjectId, Arm64FunctionTarget, Arm64Instruction, Arm64LoadStoreSize,
    Arm64NocterAbi, Arm64Register, Arm64SelectedFloatRegister, Arm64SelectedRegister,
    Arm64SelectedTerminator, Arm64ValueStorage,
};

use crate::async_function::Arm64AsyncResumeResources;

pub(crate) fn materialize(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
    resources: Arm64AsyncResumeResources<'_>,
) -> Result<Arm64Code, Arm64AsyncResumeError> {
    validate_target(plan, target)?;
    let selected = plan.selected();
    let mut code = Arm64CodeBuilder::new();
    let labels = selected
        .blocks()
        .map(|(block, _)| (block, code.create_label()))
        .collect::<Vec<_>>();
    let initial = code.create_label();
    let state_labels = plan
        .activation()
        .states()
        .iter()
        .map(|state| (state.suspend(), code.create_label()))
        .collect::<Vec<_>>();

    Arm64FrameCode::emit_prologue(selected.frame().layout(), &mut code);
    store_async_frame(plan, argument(0)?, &mut code)?;
    restore_ambient_context(plan, &mut code)?;
    emit_state_dispatch(plan, initial, &state_labels, &mut code)?;

    code.bind(initial)?;
    restore_fields(plan, plan.activation().initial(), &mut code)?;
    emit_selected_entry(plan, resources, &mut code)?;
    code.branch(block_label(&labels, selected.entry())?, false);

    for state in plan.activation().states() {
        let label = state_label(&state_labels, state.suspend())?;
        code.bind(label)?;
        restore_fields(plan, state.fields(), &mut code)?;
        emit_poll(plan, state, &labels, false, &mut code)?;
    }

    let context = selected_context(plan, resources);
    for (block_id, block) in selected.blocks() {
        code.bind(block_label(&labels, block_id)?)?;
        for instruction in block.instructions() {
            crate::selected_code::emit_instruction(context, instruction, &mut code)?;
        }
        match block.terminator() {
            Arm64SelectedTerminator::Suspend { .. } => {
                let state = plan
                    .activation()
                    .state(block_id)
                    .ok_or(Arm64AsyncResumeError::MissingState(block_id))?;
                emit_poll(plan, state, &labels, true, &mut code)?;
            }
            Arm64SelectedTerminator::DeferredReturn(value) => {
                emit_completion(plan, *value, &mut code)?;
            }
            terminator => {
                crate::selected_code::emit_terminator(selected, terminator, &labels, &mut code)?;
            }
        }
    }
    code.finish().map_err(Arm64AsyncResumeError::Code)
}

fn validate_target(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
) -> Result<(), Arm64AsyncResumeError> {
    if target.owner() != plan.owner() {
        return Err(Arm64AsyncResumeError::ForeignTarget {
            expected: plan.owner(),
            actual: target.owner(),
        });
    }
    target
        .asynchronous()
        .ok_or(Arm64AsyncResumeError::ImmediateTarget(plan.owner()))?;
    Ok(())
}

fn selected_context<'a>(
    plan: &'a Arm64AsyncFunctionPlan,
    resources: Arm64AsyncResumeResources<'a>,
) -> crate::selected_code::InstructionMaterialization<'a> {
    crate::selected_code::InstructionMaterialization {
        function: plan.selected(),
        functions: resources.functions(),
        async_primitives: resources.async_primitives(),
        data: resources.data(),
        imports: resources.imports(),
        pack_callbacks: resources.pack_callbacks(),
        allocation_failure_error: resources.allocation_failure_error(),
    }
}

fn emit_selected_entry(
    plan: &Arm64AsyncFunctionPlan,
    resources: Arm64AsyncResumeResources<'_>,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let context = selected_context(plan, resources);
    for instruction in plan.selected().entry_instructions() {
        crate::selected_code::emit_instruction(context, instruction, code)?;
    }
    Ok(())
}

fn emit_state_dispatch(
    plan: &Arm64AsyncFunctionPlan,
    initial: crate::Arm64LabelId,
    states: &[(nocter_machine::MachineBlockId, crate::Arm64LabelId)],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let mapping = load_async_frame(plan, scratch(1)?, code)?;
    let actual = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        actual,
        mapping,
        plan.frame().state_tag().offset(),
    );
    emit_tag_branch(actual, plan.frame().initial_tag(), initial, code)?;
    for state in plan.activation().states() {
        emit_tag_branch(
            actual,
            state.tag(),
            state_label(states, state.suspend())?,
            code,
        )?;
    }
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });
    Ok(())
}

fn emit_tag_branch(
    actual: Arm64Register,
    tag: u64,
    target: crate::Arm64LabelId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    // State inspection must not overwrite the asynchronous result registers. In
    // particular, x1 and x2 carry the interest slice after a child resume.
    let expected = scratch(1)?;
    crate::frame_access::load_immediate(code, expected, tag, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(actual),
        right: Arm64DataRegister::General(expected),
    });
    code.branch_conditional(target, Arm64BranchCondition::Equal);
    Ok(())
}

fn emit_poll(
    plan: &Arm64AsyncFunctionPlan,
    state: &Arm64AsyncActivationState,
    labels: &[(nocter_machine::MachineBlockId, crate::Arm64LabelId)],
    save_on_pending: bool,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let block = plan
        .selected()
        .block(state.suspend())
        .ok_or(Arm64AsyncResumeError::MissingState(state.suspend()))?;
    let Arm64SelectedTerminator::Suspend {
        computation,
        resume,
        result,
    } = block.terminator()
    else {
        return Err(Arm64AsyncResumeError::MissingState(state.suspend()));
    };
    let child = crate::selected_code::read_register(plan.selected(), *computation, 0, code)?;
    crate::address_code::move_register(code, child, argument(0)?);
    let resume_entry = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        resume_entry,
        argument(0)?,
        Arm64NocterAbi::asynchronous().resume_function_offset(),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: resume_entry,
        link: true,
    });

    let completed = code.create_label();
    let pending = code.create_label();
    emit_tag_branch(
        argument(0)?,
        Arm64NocterAbi::asynchronous().completed_status(),
        completed,
        code,
    )?;
    emit_tag_branch(
        argument(0)?,
        Arm64NocterAbi::asynchronous().pending_status(),
        pending,
        code,
    )?;
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });

    code.bind(pending)?;
    if save_on_pending {
        stage_interests(plan, code)?;
        save_fields(plan, state.fields(), code)?;
        write_state_tag(plan, state.tag(), code)?;
        restore_interests(plan, code)?;
        crate::frame_access::load_immediate(
            code,
            argument(0)?,
            Arm64NocterAbi::asynchronous().pending_status(),
            Arm64DataSize::Bits64,
        );
    }
    Arm64FrameCode::emit_epilogue(plan.selected().frame().layout(), code);

    code.bind(completed)?;
    consume_child_output(plan, *computation, *result, code)?;
    code.branch(block_label(labels, *resume)?, false);
    Ok(())
}

fn consume_child_output(
    plan: &Arm64AsyncFunctionPlan,
    computation: Arm64SelectedRegister,
    result: nocter_machine::MachineValueId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    // Form the destination before retaining the indirect branch target. A large activation frame
    // may need the compiler scratch bank to materialize this stack address.
    if let Some(staging) = plan.selected().frame().async_output_staging() {
        form_object_address(plan, staging, argument(1)?, code)?;
    } else {
        crate::frame_access::load_immediate(code, argument(1)?, 0, Arm64DataSize::Bits64);
    }
    let child = crate::selected_code::read_register(plan.selected(), computation, 0, code)?;
    crate::address_code::move_register(code, child, argument(0)?);
    let consume_entry = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        consume_entry,
        argument(0)?,
        Arm64NocterAbi::asynchronous().consume_function_offset(),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: consume_entry,
        link: true,
    });
    restore_value_from_staging(plan, result, code)
}

fn emit_completion(
    plan: &Arm64AsyncFunctionPlan,
    value: Option<nocter_machine::MachineValueId>,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    match (value, plan.frame().output()) {
        (Some(value), Some(output)) => save_value_to_heap(plan, value, output, code)?,
        (None, None) => {}
        _ => return Err(Arm64AsyncResumeError::OutputShape),
    }
    write_state_tag(plan, plan.frame().completed_tag(), code)?;
    let schema = Arm64NocterAbi::asynchronous();
    crate::frame_access::load_immediate(
        code,
        abi_register(schema.status_result_register())?,
        schema.completed_status(),
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(
        code,
        abi_register(schema.interests_pointer_result_register())?,
        0,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(
        code,
        abi_register(schema.interest_count_result_register())?,
        0,
        Arm64DataSize::Bits64,
    );
    Arm64FrameCode::emit_epilogue(plan.selected().frame().layout(), code);
    Ok(())
}

fn restore_fields(
    plan: &Arm64AsyncFunctionPlan,
    fields: &[Arm64AsyncActivationField],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    for field in fields {
        match field.transient() {
            Arm64AsyncActivationTarget::DropFlag(target)
            | Arm64AsyncActivationTarget::Pack(target) => {
                copy_heap_to_object(plan, field.persistent(), target, code)?;
            }
            Arm64AsyncActivationTarget::Value(value) => {
                restore_value_from_heap(plan, value, field.persistent(), code)?;
            }
        }
    }
    Ok(())
}

fn save_fields(
    plan: &Arm64AsyncFunctionPlan,
    fields: &[Arm64AsyncActivationField],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    for field in fields {
        match field.transient() {
            Arm64AsyncActivationTarget::DropFlag(source)
            | Arm64AsyncActivationTarget::Pack(source) => {
                copy_object_to_heap(plan, source, field.persistent(), code)?;
            }
            Arm64AsyncActivationTarget::Value(value) => {
                save_value_to_heap(plan, value, field.persistent(), code)?;
            }
        }
    }
    Ok(())
}

fn restore_value_from_heap(
    plan: &Arm64AsyncFunctionPlan,
    value: nocter_machine::MachineValueId,
    source: Arm64AsyncFrameField,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    match value_storage(plan, value)? {
        Arm64ValueStorage::Omitted if source.size() == 0 => Ok(()),
        Arm64ValueStorage::Direct(registers) => {
            for (lane, register) in registers.iter().copied().enumerate() {
                let offset = u64::try_from(lane)
                    .map_err(|_| Arm64AsyncResumeError::OffsetOverflow)?
                    * Arm64NocterAbi::word_size();
                let bytes = lane_bytes(source.size(), offset)?;
                let destination = crate::selected_code::write_target(
                    plan.selected(),
                    Arm64SelectedRegister::Virtual(register),
                )?;
                load_heap_width(plan, source, offset, bytes, destination.register, code)?;
                crate::selected_code::finish_write(destination, code);
            }
            Ok(())
        }
        Arm64ValueStorage::Floating { register, bytes } => {
            let transfer = scratch(0)?;
            load_heap_width(plan, source, 0, *bytes, transfer, code)?;
            crate::floating_code::emit_from_bits(
                plan.selected(),
                Arm64SelectedFloatRegister::Virtual(*register),
                Arm64SelectedRegister::Fixed(transfer),
                data_size(*bytes)?,
                code,
            )?;
            Ok(())
        }
        Arm64ValueStorage::Memory { size, .. } => {
            let target = plan
                .selected()
                .frame()
                .memory_value(value)
                .ok_or(Arm64AsyncResumeError::MissingMemoryValue(value))?;
            require_size(source.size(), *size)?;
            copy_heap_to_object(plan, source, target, code)
        }
        Arm64ValueStorage::Omitted => Err(Arm64AsyncResumeError::ValueShape(value)),
    }
}

fn save_value_to_heap(
    plan: &Arm64AsyncFunctionPlan,
    value: nocter_machine::MachineValueId,
    destination: Arm64AsyncFrameField,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    match value_storage(plan, value)? {
        Arm64ValueStorage::Omitted if destination.size() == 0 => Ok(()),
        Arm64ValueStorage::Direct(registers) => {
            for (lane, register) in registers.iter().copied().enumerate() {
                let offset = u64::try_from(lane)
                    .map_err(|_| Arm64AsyncResumeError::OffsetOverflow)?
                    * Arm64NocterAbi::word_size();
                let bytes = lane_bytes(destination.size(), offset)?;
                let source = crate::selected_code::read_register(
                    plan.selected(),
                    Arm64SelectedRegister::Virtual(register),
                    0,
                    code,
                )?;
                store_heap_width(plan, destination, offset, bytes, source, code)?;
            }
            Ok(())
        }
        Arm64ValueStorage::Floating { register, bytes } => {
            let transfer = scratch(0)?;
            crate::floating_code::emit_to_bits(
                plan.selected(),
                Arm64SelectedRegister::Fixed(transfer),
                Arm64SelectedFloatRegister::Virtual(*register),
                data_size(*bytes)?,
                code,
            )?;
            store_heap_width(plan, destination, 0, *bytes, transfer, code)
        }
        Arm64ValueStorage::Memory { size, .. } => {
            let source = plan
                .selected()
                .frame()
                .memory_value(value)
                .ok_or(Arm64AsyncResumeError::MissingMemoryValue(value))?;
            require_size(destination.size(), *size)?;
            copy_object_to_heap(plan, source, destination, code)
        }
        Arm64ValueStorage::Omitted => Err(Arm64AsyncResumeError::ValueShape(value)),
    }
}

fn restore_value_from_staging(
    plan: &Arm64AsyncFunctionPlan,
    value: nocter_machine::MachineValueId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let storage = value_storage(plan, value)?;
    if matches!(storage, Arm64ValueStorage::Omitted) {
        return Ok(());
    }
    let staging = plan
        .selected()
        .frame()
        .async_output_staging()
        .ok_or(Arm64AsyncResumeError::MissingOutputStaging)?;
    let staging_offset = object(plan, staging)?.offset();
    match storage {
        Arm64ValueStorage::Direct(registers) => {
            let size = value_size(plan, value)?;
            for (lane, register) in registers.iter().copied().enumerate() {
                let offset = u64::try_from(lane)
                    .map_err(|_| Arm64AsyncResumeError::OffsetOverflow)?
                    * Arm64NocterAbi::word_size();
                let bytes = lane_bytes(size, offset)?;
                let destination = crate::selected_code::write_target(
                    plan.selected(),
                    Arm64SelectedRegister::Virtual(register),
                )?;
                load_stack_width(
                    checked_add(staging_offset, offset)?,
                    bytes,
                    destination.register,
                    code,
                )?;
                crate::selected_code::finish_write(destination, code);
            }
            Ok(())
        }
        Arm64ValueStorage::Floating { register, bytes } => {
            let transfer = scratch(0)?;
            load_stack_width(staging_offset, *bytes, transfer, code)?;
            crate::floating_code::emit_from_bits(
                plan.selected(),
                Arm64SelectedFloatRegister::Virtual(*register),
                Arm64SelectedRegister::Fixed(transfer),
                data_size(*bytes)?,
                code,
            )?;
            Ok(())
        }
        Arm64ValueStorage::Memory { size, .. } => {
            let target = plan
                .selected()
                .frame()
                .memory_value(value)
                .ok_or(Arm64AsyncResumeError::MissingMemoryValue(value))?;
            copy_object_to_object(plan, staging, target, *size, code)
        }
        Arm64ValueStorage::Omitted => Ok(()),
    }
}

fn value_storage(
    plan: &Arm64AsyncFunctionPlan,
    value: nocter_machine::MachineValueId,
) -> Result<&Arm64ValueStorage, Arm64AsyncResumeError> {
    plan.selected()
        .values()
        .value(value)
        .ok_or(Arm64AsyncResumeError::UnknownValue(value))
}

fn value_size(
    plan: &Arm64AsyncFunctionPlan,
    value: nocter_machine::MachineValueId,
) -> Result<u64, Arm64AsyncResumeError> {
    match plan.activation().value(value) {
        Some(nocter_machine::MachineValueRepresentation::Stored { size, .. }) => Ok(size),
        Some(
            nocter_machine::MachineValueRepresentation::Completion
            | nocter_machine::MachineValueRepresentation::Diverging,
        )
        | None => Err(Arm64AsyncResumeError::UnknownValue(value)),
    }
}

fn copy_heap_to_object(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64AsyncFrameField,
    target: Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let target = object(plan, target)?;
    require_size(source.size(), target.size())?;
    for (offset, bytes) in crate::memory_code::exact_memory_chunks(source.size()) {
        let transfer = scratch(0)?;
        let mapping = load_async_frame(plan, scratch(1)?, code)?;
        crate::address_code::load_native(
            code,
            native_size(bytes)?,
            None,
            transfer,
            mapping,
            checked_add(source.offset(), offset)?,
        );
        crate::frame_access::store_at_stack_offset(
            code,
            native_size(bytes)?,
            transfer,
            checked_add(target.offset(), offset)?,
        );
    }
    Ok(())
}

fn copy_object_to_heap(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64FrameObjectId,
    target: Arm64AsyncFrameField,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let source = object(plan, source)?;
    require_size(source.size(), target.size())?;
    for (offset, bytes) in crate::memory_code::exact_memory_chunks(source.size()) {
        let transfer = scratch(0)?;
        crate::frame_access::load_at_stack_offset(
            code,
            native_size(bytes)?,
            transfer,
            checked_add(source.offset(), offset)?,
        );
        let mapping = load_async_frame(plan, scratch(1)?, code)?;
        crate::address_code::store_native(
            code,
            native_size(bytes)?,
            transfer,
            mapping,
            checked_add(target.offset(), offset)?,
        );
    }
    Ok(())
}

fn copy_object_to_object(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64FrameObjectId,
    target: Arm64FrameObjectId,
    size: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let source = object(plan, source)?;
    let target = object(plan, target)?;
    if source.size() < size || target.size() < size {
        return Err(Arm64AsyncResumeError::OutputShape);
    }
    for (offset, bytes) in crate::memory_code::exact_memory_chunks(size) {
        let transfer = scratch(0)?;
        crate::frame_access::load_at_stack_offset(
            code,
            native_size(bytes)?,
            transfer,
            checked_add(source.offset(), offset)?,
        );
        crate::frame_access::store_at_stack_offset(
            code,
            native_size(bytes)?,
            transfer,
            checked_add(target.offset(), offset)?,
        );
    }
    Ok(())
}

fn load_heap_width(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64AsyncFrameField,
    offset: u64,
    bytes: u8,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let mapping = load_async_frame(plan, scratch_avoiding(destination)?, code)?;
    let offset = checked_add(source.offset(), offset)?;
    if let Ok(size) = native_size(bytes) {
        crate::address_code::load_native(code, size, None, destination, mapping, offset);
    } else {
        crate::address_code::emit_fragmented_load(code, bytes, destination, mapping, offset);
    }
    Ok(())
}

fn store_heap_width(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64AsyncFrameField,
    offset: u64,
    bytes: u8,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let mapping = load_async_frame(plan, scratch_avoiding(source)?, code)?;
    let offset = checked_add(target.offset(), offset)?;
    if let Ok(size) = native_size(bytes) {
        crate::address_code::store_native(code, size, source, mapping, offset);
    } else {
        crate::address_code::emit_fragmented_store(code, bytes, source, mapping, offset);
    }
    Ok(())
}

fn load_stack_width(
    offset: u64,
    bytes: u8,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    if let Ok(size) = native_size(bytes) {
        crate::frame_access::load_at_stack_offset(code, size, destination, offset);
    } else {
        crate::memory_code::emit_fragmented_load(code, bytes, destination, offset)?;
    }
    Ok(())
}

fn restore_ambient_context(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let mapping = load_async_frame(plan, scratch(1)?, code)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        Arm64NocterAbi::allocation_context_register(),
        mapping,
        plan.frame().allocation_context().offset(),
    );
    match plan.selected().frame().allocation_context() {
        crate::Arm64AllocationContextFrame::IncomingPointer(object) => store_object_word(
            plan,
            object,
            Arm64NocterAbi::allocation_context_register(),
            code,
        )?,
        crate::Arm64AllocationContextFrame::None
        | crate::Arm64AllocationContextFrame::ProgramRoot(_) => {
            return Err(Arm64AsyncResumeError::ContextShape);
        }
    }
    match (
        plan.frame().process_context(),
        plan.selected().frame().process_context(),
    ) {
        (Some(source), crate::Arm64ProcessContextFrame::IncomingPointer(object)) => {
            let mapping = load_async_frame(plan, scratch(1)?, code)?;
            crate::address_code::load_native(
                code,
                Arm64LoadStoreSize::Double,
                None,
                Arm64NocterAbi::process_context_register(),
                mapping,
                source.offset(),
            );
            store_object_word(
                plan,
                object,
                Arm64NocterAbi::process_context_register(),
                code,
            )?;
        }
        (None, crate::Arm64ProcessContextFrame::None) => {}
        _ => return Err(Arm64AsyncResumeError::ContextShape),
    }
    Ok(())
}

fn stage_interests(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let schema = Arm64NocterAbi::asynchronous();
    store_object_word(
        plan,
        plan.selected()
            .frame()
            .async_interest_pointer()
            .ok_or(Arm64AsyncResumeError::MissingInterestStaging)?,
        abi_register(schema.interests_pointer_result_register())?,
        code,
    )?;
    store_object_word(
        plan,
        plan.selected()
            .frame()
            .async_interest_count()
            .ok_or(Arm64AsyncResumeError::MissingInterestStaging)?,
        abi_register(schema.interest_count_result_register())?,
        code,
    )
}

fn restore_interests(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let schema = Arm64NocterAbi::asynchronous();
    load_object_word(
        plan,
        plan.selected()
            .frame()
            .async_interest_pointer()
            .ok_or(Arm64AsyncResumeError::MissingInterestStaging)?,
        abi_register(schema.interests_pointer_result_register())?,
        code,
    )?;
    load_object_word(
        plan,
        plan.selected()
            .frame()
            .async_interest_count()
            .ok_or(Arm64AsyncResumeError::MissingInterestStaging)?,
        abi_register(schema.interest_count_result_register())?,
        code,
    )
}

fn write_state_tag(
    plan: &Arm64AsyncFunctionPlan,
    tag: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let value = scratch(0)?;
    crate::frame_access::load_immediate(code, value, tag, Arm64DataSize::Bits64);
    let mapping = load_async_frame(plan, scratch(1)?, code)?;
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        value,
        mapping,
        plan.frame().state_tag().offset(),
    );
    Ok(())
}

fn store_async_frame(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    let object = plan
        .selected()
        .frame()
        .async_frame_pointer()
        .ok_or(Arm64AsyncResumeError::MissingFramePointer)?;
    store_object_word(plan, object, source, code)
}

fn load_async_frame(
    plan: &Arm64AsyncFunctionPlan,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64Register, Arm64AsyncResumeError> {
    let object = plan
        .selected()
        .frame()
        .async_frame_pointer()
        .ok_or(Arm64AsyncResumeError::MissingFramePointer)?;
    load_object_word(plan, object, destination, code)?;
    Ok(destination)
}

fn store_object_word(
    plan: &Arm64AsyncFunctionPlan,
    object_id: Arm64FrameObjectId,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        source,
        object(plan, object_id)?.offset(),
    );
    Ok(())
}

fn load_object_word(
    plan: &Arm64AsyncFunctionPlan,
    object_id: Arm64FrameObjectId,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        object(plan, object_id)?.offset(),
    );
    Ok(())
}

fn form_object_address(
    plan: &Arm64AsyncFunctionPlan,
    object_id: Arm64FrameObjectId,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncResumeError> {
    crate::frame_access::form_stack_address(code, destination, object(plan, object_id)?.offset());
    Ok(())
}

fn object(
    plan: &Arm64AsyncFunctionPlan,
    id: Arm64FrameObjectId,
) -> Result<crate::Arm64FrameObject, Arm64AsyncResumeError> {
    plan.selected()
        .frame()
        .layout()
        .object(id)
        .ok_or(Arm64AsyncResumeError::UnknownFrameObject(id))
}

fn block_label(
    labels: &[(nocter_machine::MachineBlockId, crate::Arm64LabelId)],
    block: nocter_machine::MachineBlockId,
) -> Result<crate::Arm64LabelId, Arm64AsyncResumeError> {
    labels
        .get(block.index())
        .and_then(|(actual, label)| (*actual == block).then_some(*label))
        .ok_or(Arm64AsyncResumeError::UnknownBlock(block))
}

fn state_label(
    labels: &[(nocter_machine::MachineBlockId, crate::Arm64LabelId)],
    block: nocter_machine::MachineBlockId,
) -> Result<crate::Arm64LabelId, Arm64AsyncResumeError> {
    labels
        .iter()
        .find_map(|(actual, label)| (*actual == block).then_some(*label))
        .ok_or(Arm64AsyncResumeError::MissingState(block))
}

fn lane_bytes(size: u64, offset: u64) -> Result<u8, Arm64AsyncResumeError> {
    let bytes = size
        .checked_sub(offset)
        .ok_or(Arm64AsyncResumeError::OutputShape)?
        .min(Arm64NocterAbi::word_size());
    u8::try_from(bytes).map_err(|_| Arm64AsyncResumeError::OutputShape)
}

fn require_size(actual: u64, expected: u64) -> Result<(), Arm64AsyncResumeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(Arm64AsyncResumeError::OutputShape)
    }
}

fn native_size(bytes: u8) -> Result<Arm64LoadStoreSize, Arm64AsyncResumeError> {
    match bytes {
        1 => Ok(Arm64LoadStoreSize::Byte),
        2 => Ok(Arm64LoadStoreSize::Half),
        4 => Ok(Arm64LoadStoreSize::Word),
        8 => Ok(Arm64LoadStoreSize::Double),
        _ => Err(Arm64AsyncResumeError::InvalidMemoryWidth(bytes)),
    }
}

fn data_size(bytes: u8) -> Result<Arm64DataSize, Arm64AsyncResumeError> {
    match bytes {
        4 => Ok(Arm64DataSize::Bits32),
        8 => Ok(Arm64DataSize::Bits64),
        _ => Err(Arm64AsyncResumeError::InvalidMemoryWidth(bytes)),
    }
}

fn scratch_avoiding(register: Arm64Register) -> Result<Arm64Register, Arm64AsyncResumeError> {
    for index in 0..2 {
        let candidate = scratch(index)?;
        if candidate != register {
            return Ok(candidate);
        }
    }
    Err(Arm64AsyncResumeError::RegisterOverflow)
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64AsyncResumeError> {
    left.checked_add(right)
        .ok_or(Arm64AsyncResumeError::OffsetOverflow)
}

fn abi_register(index: u8) -> Result<Arm64Register, Arm64AsyncResumeError> {
    Arm64NocterAbi::argument_register(index).ok_or(Arm64AsyncResumeError::RegisterOverflow)
}

fn argument(index: u8) -> Result<Arm64Register, Arm64AsyncResumeError> {
    abi_register(index)
}

fn scratch(index: u8) -> Result<Arm64Register, Arm64AsyncResumeError> {
    Arm64NocterAbi::compiler_scratch_register(index).ok_or(Arm64AsyncResumeError::RegisterOverflow)
}
