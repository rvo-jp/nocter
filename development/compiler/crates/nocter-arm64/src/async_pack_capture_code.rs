use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncConstructorError,
    Arm64AsyncFunctionPlan, Arm64BaseRegister, Arm64BranchCondition, Arm64CodeBuilder,
    Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi, Arm64PackDescriptorLayout,
    Arm64Register,
};

/// Saves the caller-owned descriptor pointer before constructor allocation clobbers ABI inputs.
pub(crate) fn stage_input(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let Some(pack) = plan.pack() else {
        return Ok(());
    };
    let object = pack_object(plan)?;
    let source = Arm64NocterAbi::argument_register(pack.transport().pointer().first())
        .ok_or(Arm64AsyncConstructorError::RegisterOverflow)?;
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        source,
        object_offset(plan, object)?,
    );
    Ok(())
}

/// Moves the descriptor and its opaque callback state into one allocation-backed owner.
pub(crate) fn capture(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let Some(pack) = plan.pack() else {
        return Ok(());
    };
    let source = argument(2)?;
    load_source_descriptor(plan, source, code)?;
    let clone_stack_pack = code.create_label();
    let complete = code.create_label();
    let existing_allocation_size = argument(1)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        existing_allocation_size,
        source,
        Arm64PackDescriptorLayout::ALLOCATION_SIZE_OFFSET,
    );
    compare_zero(existing_allocation_size, code);
    code.branch_conditional(clone_stack_pack, Arm64BranchCondition::Equal);
    store_pack_pointer(plan, pack.destination(), source, code)?;
    code.branch(complete, false);

    code.bind(clone_stack_pack)?;
    let state_size = argument(1)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        state_size,
        source,
        Arm64PackDescriptorLayout::STATE_SIZE_OFFSET,
    );
    crate::address_code::add_offset(code, state_size, Arm64PackDescriptorLayout::SIZE);
    crate::darwin_memory_code::emit_map(code)?;

    let allocation = argument(0)?;
    store_pack_pointer(plan, pack.destination(), allocation, code)?;

    load_source_descriptor(plan, source, code)?;
    copy_descriptor(source, allocation, code)?;
    let remaining = argument(5)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        remaining,
        source,
        Arm64PackDescriptorLayout::STATE_SIZE_OFFSET,
    );
    let allocation_size = argument(1)?;
    crate::address_code::move_register(code, remaining, allocation_size);
    crate::address_code::add_offset(code, allocation_size, Arm64PackDescriptorLayout::SIZE);
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        allocation_size,
        allocation,
        Arm64PackDescriptorLayout::ALLOCATION_SIZE_OFFSET,
    );

    let source_state = argument(3)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        source_state,
        source,
        Arm64PackDescriptorLayout::STATE_POINTER_OFFSET,
    );
    let destination_state = argument(4)?;
    crate::address_code::move_register(code, allocation, destination_state);
    crate::address_code::add_offset(code, destination_state, Arm64PackDescriptorLayout::SIZE);
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        destination_state,
        allocation,
        Arm64PackDescriptorLayout::STATE_POINTER_OFFSET,
    );
    copy_dynamic_state(source_state, destination_state, remaining, code)?;
    code.bind(complete)?;
    Ok(())
}

fn store_pack_pointer(
    plan: &Arm64AsyncFunctionPlan,
    destination: crate::Arm64AsyncFrameField,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let frame = load_parent_frame(plan, scratch(1)?, code)?;
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        source,
        frame,
        destination.offset(),
    );
    Ok(())
}

fn copy_descriptor(
    source: Arm64Register,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    for (offset, bytes) in crate::memory_code::exact_memory_chunks(Arm64PackDescriptorLayout::SIZE)
    {
        let transfer = argument(6)?;
        let size = native_size(bytes)?;
        crate::address_code::load_native(code, size, None, transfer, source, offset);
        crate::address_code::store_native(code, size, transfer, destination, offset);
    }
    Ok(())
}

fn copy_dynamic_state(
    source: Arm64Register,
    destination: Arm64Register,
    remaining: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let loop_ = code.create_label();
    let complete = code.create_label();
    code.bind(loop_)?;
    compare_zero(remaining, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    let transfer = argument(6)?;
    crate::address_code::load_native(code, Arm64LoadStoreSize::Byte, None, transfer, source, 0);
    crate::address_code::store_native(code, Arm64LoadStoreSize::Byte, transfer, destination, 0);
    crate::address_code::add_offset(code, source, 1);
    crate::address_code::add_offset(code, destination, 1);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(remaining),
        source: Arm64BaseRegister::General(remaining),
        immediate: 1,
        shift_12: false,
    });
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn compare_zero(register: Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register),
        immediate: 0,
        shift_12: false,
    });
}

fn load_source_descriptor(
    plan: &Arm64AsyncFunctionPlan,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        object_offset(plan, pack_object(plan)?)?,
    );
    Ok(())
}

fn load_parent_frame(
    plan: &Arm64AsyncFunctionPlan,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        object_offset(plan, plan.constructor_frame().mapping())?,
    );
    Ok(destination)
}

fn pack_object(
    plan: &Arm64AsyncFunctionPlan,
) -> Result<crate::Arm64FrameObjectId, Arm64AsyncConstructorError> {
    plan.constructor_frame()
        .pack_input()
        .ok_or(Arm64AsyncConstructorError::MissingPackInput)
}

fn object_offset(
    plan: &Arm64AsyncFunctionPlan,
    id: crate::Arm64FrameObjectId,
) -> Result<u64, Arm64AsyncConstructorError> {
    plan.constructor_frame()
        .layout()
        .object(id)
        .map(crate::Arm64FrameObject::offset)
        .ok_or(Arm64AsyncConstructorError::UnknownFrameObject(id))
}

fn native_size(bytes: u8) -> Result<Arm64LoadStoreSize, Arm64AsyncConstructorError> {
    match bytes {
        1 => Ok(Arm64LoadStoreSize::Byte),
        2 => Ok(Arm64LoadStoreSize::Half),
        4 => Ok(Arm64LoadStoreSize::Word),
        8 => Ok(Arm64LoadStoreSize::Double),
        _ => Err(Arm64AsyncConstructorError::InvalidMemoryWidth(bytes)),
    }
}

fn argument(index: u8) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    Arm64NocterAbi::argument_register(index).ok_or(Arm64AsyncConstructorError::RegisterOverflow)
}

fn scratch(index: u8) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    Arm64NocterAbi::compiler_scratch_register(index)
        .ok_or(Arm64AsyncConstructorError::RegisterOverflow)
}
