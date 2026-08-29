use crate::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction, Arm64LoadStoreSize, Arm64MaterializationError, Arm64NocterAbi,
    Arm64SelectedFunction,
};

use crate::region_layout::Arm64RegionLayout;

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_MUNMAP: u64 = 0x0200_0049;

pub(crate) fn emit_create(
    function: &Arm64SelectedFunction,
    region: crate::Arm64FrameObjectId,
    parent: crate::Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    emit_create_at(function, region_offset(function, region)?, parent, code)
}

fn emit_create_at(
    function: &Arm64SelectedFunction,
    offset: u64,
    parent: crate::Arm64SelectedRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let state = scratch(0);
    let value = scratch(1);
    let parent = crate::selected_code::read_register(function, parent, 0, code)?;
    load_word(code, value, parent, 0);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        value,
        checked_add(offset, Arm64RegionLayout::PARENT_STATE_OFFSET)?,
    );
    load_word(code, value, parent, Arm64NocterAbi::word_size());
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        value,
        checked_add(offset, Arm64RegionLayout::PARENT_KIND_OFFSET)?,
    );
    crate::frame_access::form_stack_address(
        code,
        state,
        checked_add(offset, Arm64RegionLayout::HEAD_OFFSET)?,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        state,
        checked_add(offset, Arm64RegionLayout::STATE_OFFSET)?,
    );
    crate::frame_access::load_immediate(
        code,
        state,
        Arm64RegionLayout::ALLOCATOR_KIND,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        state,
        checked_add(offset, Arm64RegionLayout::KIND_OFFSET)?,
    );
    crate::frame_access::load_immediate(code, state, 0, Arm64DataSize::Bits64);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        state,
        checked_add(offset, Arm64RegionLayout::HEAD_OFFSET)?,
    );
    Ok(())
}

pub(crate) fn emit_release(
    function: &Arm64SelectedFunction,
    region: crate::Arm64FrameObjectId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    emit_release_at(region_offset(function, region)?, code)
}

fn emit_release_at(
    offset: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let loop_ = code.create_label();
    let complete = code.create_label();
    let mapping = argument(0);
    let size = argument(1);
    let temporary = scratch(0);
    let head = checked_add(offset, Arm64RegionLayout::HEAD_OFFSET)?;

    code.bind(loop_)?;
    crate::frame_access::load_at_stack_offset(code, Arm64LoadStoreSize::Double, mapping, head);
    compare_zero(code, mapping);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    load_word(code, temporary, mapping, 0);
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, temporary, head);
    load_word(code, size, mapping, Arm64NocterAbi::word_size());
    crate::frame_access::load_immediate(code, temporary, DARWIN_MUNMAP, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    code.branch_conditional(loop_, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::RegionReleaseFailure.immediate(),
    });
    code.bind(complete)?;
    Ok(())
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: crate::Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: crate::Arm64AddSubtractDestination::Zero,
        source: crate::Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn load_word(
    code: &mut Arm64CodeBuilder,
    destination: crate::Arm64Register,
    base: crate::Arm64Register,
    offset: u64,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: crate::Arm64BaseRegister::General(base),
        offset: u32::try_from(offset).expect("region header offsets fit the instruction"),
    });
}

fn region_offset(
    function: &Arm64SelectedFunction,
    region: crate::Arm64FrameObjectId,
) -> Result<u64, Arm64MaterializationError> {
    let object = function
        .frame()
        .layout()
        .object(region)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(region))?;
    if object.size() != Arm64RegionLayout::SIZE
        || object.alignment() != Arm64RegionLayout::ALIGNMENT
    {
        return Err(Arm64MaterializationError::InvalidRegionFrame(region));
    }
    Ok(object.offset())
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64MaterializationError> {
    left.checked_add(right)
        .ok_or(Arm64MaterializationError::OffsetOverflow)
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("region release uses ABI argument registers")
}

fn scratch(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(index)
        .expect("the ABI reserves compiler scratch registers")
}
