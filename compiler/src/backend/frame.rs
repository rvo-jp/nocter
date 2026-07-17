use crate::abi::{ReturnPassing, ValueLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, BorrowSource, FallibleFailureMode, Function,
    I32Location, I32Value, Instruction, ScalarArgument, SliceLocation, SliceValue, StrLocation,
    StrValue, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FunctionFrame {
    Frameless,
    Framed(FrameLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrameLayout {
    frame_size: u32,
    saved_x30_offset: u32,
    parameter_spill_slots: Vec<ParameterSpillSlot>,
    scalar_spill_slots: Vec<ScalarSpillSlot>,
    argument_staging_slots: Vec<ArgumentStagingSlot>,
    aggregate_slots: Vec<AggregateSlot>,
    indirect_return_pointer_offset: Option<u32>,
}

impl FrameLayout {
    pub(super) fn frame_size(&self) -> u32 {
        self.frame_size
    }

    pub(super) fn saved_x30_offset(&self) -> u32 {
        self.saved_x30_offset
    }

    pub(super) fn parameter_spill_slots(&self) -> &[ParameterSpillSlot] {
        &self.parameter_spill_slots
    }

    pub(super) fn parameter_spill_slot(
        &self,
        parameter_index: usize,
    ) -> Option<ParameterSpillSlot> {
        self.parameter_spill_slots
            .iter()
            .copied()
            .find(|slot| slot.parameter_index == parameter_index)
    }

    pub(super) fn scalar_spill_slots(&self) -> &[ScalarSpillSlot] {
        &self.scalar_spill_slots
    }

    pub(super) fn scalar_spill_slot(&self, local_index: usize) -> Option<ScalarSpillSlot> {
        self.scalar_spill_slots
            .iter()
            .copied()
            .find(|slot| slot.local_index == local_index)
    }

    pub(super) fn argument_staging_slots(&self) -> &[ArgumentStagingSlot] {
        &self.argument_staging_slots
    }

    #[allow(dead_code)]
    pub(super) fn aggregate_slots(&self) -> &[AggregateSlot] {
        &self.aggregate_slots
    }

    #[allow(dead_code)]
    pub(super) fn aggregate_slot(&self, slot_index: usize) -> Option<AggregateSlot> {
        self.aggregate_slots
            .iter()
            .copied()
            .find(|slot| slot.slot_index == slot_index)
    }

    pub(super) fn indirect_return_pointer_offset(&self) -> Option<u32> {
        self.indirect_return_pointer_offset
    }

    pub(super) fn for_slot_counts(
        scalar_spill_count: usize,
        argument_staging_count: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::for_slot_counts_with_parameter_spills_and_aggregate_slots(
            scalar_spill_count,
            argument_staging_count,
            &[],
            &[],
            false,
        )
    }

    #[allow(dead_code)]
    pub(super) fn for_slot_counts_with_aggregate_slots(
        scalar_spill_count: usize,
        argument_staging_count: usize,
        aggregate_slot_requests: &[AggregateSlotRequest],
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::for_slot_counts_with_parameter_spills_and_aggregate_slots(
            scalar_spill_count,
            argument_staging_count,
            &[],
            aggregate_slot_requests,
            false,
        )
    }

    fn for_slot_counts_with_parameter_spills_and_aggregate_slots(
        scalar_spill_count: usize,
        argument_staging_count: usize,
        parameter_spill_requests: &[usize],
        aggregate_slot_requests: &[AggregateSlotRequest],
        spill_indirect_return_pointer: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut parameter_spill_requests = parameter_spill_requests.to_vec();
        parameter_spill_requests.sort_unstable();
        parameter_spill_requests.dedup();
        let indirect_return_pointer_bytes = if spill_indirect_return_pointer {
            SCALAR_SLOT_SIZE
        } else {
            0
        };
        let parameter_spill_bytes = parameter_spill_requests
            .len()
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("parameter spill slot count overflows host usize")
            })?;
        let scalar_spill_bytes = scalar_spill_count
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("scalar spill slot count overflows host usize")
            })?;
        let argument_staging_bytes = argument_staging_count
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("argument staging slot count overflows host usize")
            })?;
        let scalar_slot_bytes = indirect_return_pointer_bytes
            .checked_add(parameter_spill_bytes)
            .and_then(|bytes| bytes.checked_add(scalar_spill_bytes))
            .and_then(|bytes| bytes.checked_add(argument_staging_bytes))
            .ok_or_else(|| frame_too_large_diagnostic("scalar slot bytes overflow host usize"))?;
        let (aggregate_slots, aggregate_slot_bytes) =
            aggregate_slots(aggregate_slot_requests, scalar_slot_bytes)?;
        let slot_bytes = scalar_slot_bytes
            .checked_add(aggregate_slot_bytes)
            .ok_or_else(|| frame_too_large_diagnostic("frame slot bytes overflow host usize"))?;
        let unaligned_frame_size = slot_bytes
            .checked_add(SAVED_X30_SLOT_SIZE)
            .ok_or_else(|| frame_too_large_diagnostic("frame size overflows host usize"))?;
        let frame_size = align_usize(unaligned_frame_size, STACK_ALIGNMENT);

        if frame_size > ADD_SUB_SP_IMM_MAX as usize {
            return Err(frame_too_large_diagnostic(
                "frame size exceeds ARM64 add/sub immediate range",
            ));
        }

        let saved_x30_offset = frame_size - SAVED_X30_SLOT_SIZE;
        if saved_x30_offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
            return Err(frame_too_large_diagnostic(
                "saved x30 slot exceeds ARM64 x-register load/store immediate range",
            ));
        }

        let indirect_return_pointer_offset = if spill_indirect_return_pointer {
            Some(0)
        } else {
            None
        };

        let mut parameter_spill_slots = Vec::with_capacity(parameter_spill_requests.len());
        for (slot_index, parameter_index) in parameter_spill_requests.into_iter().enumerate() {
            let offset = slot_index
                .checked_mul(SCALAR_SLOT_SIZE)
                .and_then(|offset| indirect_return_pointer_bytes.checked_add(offset))
                .ok_or_else(|| {
                    frame_too_large_diagnostic("parameter spill slot offset overflows host usize")
                })?;
            if offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "parameter spill slot exceeds ARM64 x-register load/store immediate range",
                ));
            }
            parameter_spill_slots.push(ParameterSpillSlot {
                parameter_index,
                offset: offset as u32,
            });
        }

        let mut scalar_spill_slots = Vec::with_capacity(scalar_spill_count);
        for local_index in 0..scalar_spill_count {
            let offset = indirect_return_pointer_bytes
                .checked_add(parameter_spill_bytes)
                .and_then(|bytes| {
                    local_index
                        .checked_mul(SCALAR_SLOT_SIZE)
                        .and_then(|offset| bytes.checked_add(offset))
                })
                .ok_or_else(|| {
                    frame_too_large_diagnostic("scalar spill slot offset overflows host usize")
                })?;
            if offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "scalar spill slot exceeds ARM64 x-register load/store immediate range",
                ));
            }
            scalar_spill_slots.push(ScalarSpillSlot {
                local_index,
                offset: offset as u32,
            });
        }

        let mut argument_staging_slots = Vec::with_capacity(argument_staging_count);
        for argument_index in 0..argument_staging_count {
            let offset = indirect_return_pointer_bytes
                .checked_add(parameter_spill_bytes)
                .and_then(|bytes| bytes.checked_add(scalar_spill_bytes))
                .and_then(|bytes| {
                    argument_index
                        .checked_mul(SCALAR_SLOT_SIZE)
                        .and_then(|offset| bytes.checked_add(offset))
                })
                .ok_or_else(|| {
                    frame_too_large_diagnostic("argument staging slot offset overflows host usize")
                })?;
            if offset > LDR_STR_W_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "argument staging slot exceeds ARM64 w-register load/store immediate range",
                ));
            }
            argument_staging_slots.push(ArgumentStagingSlot {
                abi_word_index: argument_index,
                offset: offset as u32,
            });
        }

        Ok(Self {
            frame_size: frame_size as u32,
            saved_x30_offset: saved_x30_offset as u32,
            parameter_spill_slots,
            scalar_spill_slots,
            argument_staging_slots,
            aggregate_slots,
            indirect_return_pointer_offset,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParameterSpillSlot {
    parameter_index: usize,
    offset: u32,
}

impl ParameterSpillSlot {
    pub(super) fn parameter_index(self) -> usize {
        self.parameter_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarSpillSlot {
    local_index: usize,
    offset: u32,
}

impl ScalarSpillSlot {
    pub(super) fn local_index(self) -> usize {
        self.local_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArgumentStagingSlot {
    abi_word_index: usize,
    offset: u32,
}

impl ArgumentStagingSlot {
    pub(super) fn abi_word_index(self) -> usize {
        self.abi_word_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AggregateSlotRequest {
    slot_index: usize,
    layout: ValueLayout,
}

#[allow(dead_code)]
impl AggregateSlotRequest {
    pub(super) fn new(slot_index: usize, layout: ValueLayout) -> Self {
        Self { slot_index, layout }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AggregateSlot {
    slot_index: usize,
    offset: u32,
    size: u32,
    align: u32,
}

#[allow(dead_code)]
impl AggregateSlot {
    pub(super) fn slot_index(self) -> usize {
        self.slot_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }

    pub(super) fn size(self) -> u32 {
        self.size
    }

    pub(super) fn align(self) -> u32 {
        self.align
    }
}

pub(super) fn plan_function_frame(function: &Function) -> Result<FunctionFrame, Vec<Diagnostic>> {
    let aggregate_slot_requests = aggregate_slot_requests(&function.instructions)?;
    let parameter_spill_requests = parameter_spill_requests(
        &function.instructions,
        function_clobbers_parameter_registers(&function.instructions),
    );
    let requires_frame = function_requires_frame(&function.instructions);
    let has_frame = requires_frame
        || !aggregate_slot_requests.is_empty()
        || !parameter_spill_requests.is_empty();
    let spill_indirect_return_pointer = has_frame
        && function.return_type.success_return_passing() == Some(ReturnPassing::IndirectPointer);

    if !has_frame {
        return Ok(FunctionFrame::Frameless);
    }

    let scalar_spill_count = scalar_spill_slot_count(&function.instructions);
    let argument_staging_count = max_call_argument_count(&function.instructions);

    if !spill_indirect_return_pointer
        && aggregate_slot_requests.is_empty()
        && parameter_spill_requests.is_empty()
    {
        return FrameLayout::for_slot_counts(scalar_spill_count, argument_staging_count)
            .map(FunctionFrame::Framed);
    }

    FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
        scalar_spill_count,
        argument_staging_count,
        &parameter_spill_requests,
        &aggregate_slot_requests,
        spill_indirect_return_pointer,
    )
    .map(FunctionFrame::Framed)
}

fn function_requires_frame(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_requires_frame)
}

fn function_clobbers_parameter_registers(instructions: &[Instruction]) -> bool {
    instructions
        .iter()
        .any(instruction_clobbers_parameter_registers)
}

fn instruction_clobbers_parameter_registers(instruction: &Instruction) -> bool {
    match instruction {
        Instruction::CallI32 { .. }
        | Instruction::CallFallibleI32 { .. }
        | Instruction::CallU8 { .. }
        | Instruction::CallFallibleU8 { .. }
        | Instruction::CallUsize { .. }
        | Instruction::CallFallibleUsize { .. }
        | Instruction::CallBool { .. }
        | Instruction::CallFallibleBool { .. }
        | Instruction::CallStr { .. }
        | Instruction::CallFallibleStr { .. }
        | Instruction::CallSlice { .. }
        | Instruction::CallFallibleSlice { .. }
        | Instruction::CallAggregate { .. }
        | Instruction::CallDirectAggregate { .. }
        | Instruction::CallFallibleDirectAggregate { .. }
        | Instruction::CallFallibleAggregate { .. }
        | Instruction::CallVoid { .. }
        | Instruction::CallFallibleVoid { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::ReadSlice { .. }
        | Instruction::OpenRead { .. }
        | Instruction::CloseFd { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::StoreU8ToPointer { .. } => true,
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            function_clobbers_parameter_registers(then_instructions)
                || function_clobbers_parameter_registers(else_instructions)
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            function_clobbers_parameter_registers(condition_instructions)
                || function_clobbers_parameter_registers(body_instructions)
        }
        Instruction::CheckFailure { failure_mode } => {
            failure_mode_clobbers_parameter_registers(failure_mode)
        }
        Instruction::TailCall { .. }
        | Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::StoreAggregateUsize { .. }
        | Instruction::StoreAggregateI32 { .. }
        | Instruction::StoreAggregateU16 { .. }
        | Instruction::StoreAggregateU32 { .. }
        | Instruction::StoreAggregateU8 { .. }
        | Instruction::StoreAggregateBool { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSlice { .. }
        | Instruction::SetSliceRawParts { .. }
        | Instruction::AddI32 { .. }
        | Instruction::SubtractI32 { .. }
        | Instruction::MultiplyI32 { .. }
        | Instruction::DivideI32 { .. }
        | Instruction::RemainderI32 { .. }
        | Instruction::ShiftLeftI32 { .. }
        | Instruction::ShiftRightI32 { .. }
        | Instruction::AddUsize { .. }
        | Instruction::SubtractUsize { .. }
        | Instruction::MultiplyUsize { .. }
        | Instruction::DivideUsize { .. }
        | Instruction::RemainderUsize { .. }
        | Instruction::ShiftLeftUsize { .. }
        | Instruction::ShiftRightUsize { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => false,
    }
}

fn failure_mode_clobbers_parameter_registers(failure_mode: &FallibleFailureMode) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Catch { instructions, .. } => {
            function_clobbers_parameter_registers(instructions)
        }
    }
}

fn instruction_requires_frame(instruction: &Instruction) -> bool {
    match instruction {
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            function_requires_frame(then_instructions) || function_requires_frame(else_instructions)
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            function_requires_frame(condition_instructions)
                || function_requires_frame(body_instructions)
        }
        Instruction::CallI32 { .. }
        | Instruction::CallFallibleI32 { .. }
        | Instruction::CallU8 { .. }
        | Instruction::CallFallibleU8 { .. }
        | Instruction::CallUsize { .. }
        | Instruction::CallFallibleUsize { .. }
        | Instruction::CallBool { .. }
        | Instruction::CallFallibleBool { .. }
        | Instruction::CallStr { .. }
        | Instruction::CallFallibleStr { .. }
        | Instruction::CallSlice { .. }
        | Instruction::CallFallibleSlice { .. }
        | Instruction::CallAggregate { .. }
        | Instruction::CallDirectAggregate { .. }
        | Instruction::CallFallibleDirectAggregate { .. }
        | Instruction::CallFallibleAggregate { .. }
        | Instruction::CallVoid { .. }
        | Instruction::CallFallibleVoid { .. }
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::ReadSlice { .. }
        | Instruction::OpenRead { .. }
        | Instruction::CloseFd { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::StoreU8ToPointer { .. } => true,
        Instruction::CopyAggregate {
            destination,
            source,
            ..
        } => {
            matches!(destination, AggregateLocation::Slot(_))
                || matches!(source, AggregateLocation::Slot(_))
        }
        Instruction::CopyAggregateRange { .. } => true,
        Instruction::StoreAggregateUsize { destination, .. }
        | Instruction::StoreAggregateI32 { destination, .. }
        | Instruction::StoreAggregateU16 { destination, .. }
        | Instruction::StoreAggregateU32 { destination, .. }
        | Instruction::StoreAggregateU8 { destination, .. }
        | Instruction::StoreAggregateBool { destination, .. } => {
            matches!(destination, AggregateLocation::Slot(_))
        }
        Instruction::LoadAggregateUsize { source, .. }
        | Instruction::LoadAggregateI32 { source, .. }
        | Instruction::LoadAggregateU8 { source, .. }
        | Instruction::LoadAggregateBool { source, .. } => {
            matches!(source, AggregateLocation::Slot(_))
        }
        Instruction::TailCall { arguments, .. } => !arguments.is_empty(),
        Instruction::CheckFailure { failure_mode } => failure_mode_requires_frame(failure_mode),
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSlice { .. }
        | Instruction::SetSliceRawParts { .. }
        | Instruction::AddI32 { .. }
        | Instruction::SubtractI32 { .. }
        | Instruction::MultiplyI32 { .. }
        | Instruction::DivideI32 { .. }
        | Instruction::RemainderI32 { .. }
        | Instruction::ShiftLeftI32 { .. }
        | Instruction::ShiftRightI32 { .. }
        | Instruction::AddUsize { .. }
        | Instruction::SubtractUsize { .. }
        | Instruction::MultiplyUsize { .. }
        | Instruction::DivideUsize { .. }
        | Instruction::RemainderUsize { .. }
        | Instruction::ShiftLeftUsize { .. }
        | Instruction::ShiftRightUsize { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => false,
    }
}

fn scalar_spill_slot_count(instructions: &[Instruction]) -> usize {
    let mut highest_local_index = None;
    record_instruction_list_scalar_locals(instructions, &mut highest_local_index);
    highest_local_index.map_or(0, |index| index + 1)
}

fn parameter_spill_requests(
    instructions: &[Instruction],
    include_value_parameters: bool,
) -> Vec<usize> {
    let mut requests = BTreeSet::new();
    record_instruction_list_parameter_spill_requests(
        instructions,
        &mut requests,
        include_value_parameters,
    );
    requests.into_iter().collect()
}

fn max_call_argument_count(instructions: &[Instruction]) -> usize {
    instructions
        .iter()
        .map(instruction_max_call_argument_count)
        .max()
        .unwrap_or(0)
}

fn instruction_max_call_argument_count(instruction: &Instruction) -> usize {
    match instruction {
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::TailCall { arguments, .. } => {
            arguments.iter().map(ScalarArgument::abi_word_count).sum()
        }
        Instruction::DarwinSyscall { arguments, .. } => arguments.len() + 1,
        Instruction::CallFallibleI32 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleU8 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleUsize {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleBool {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleStr {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleSlice {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleDirectAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleVoid {
            arguments,
            failure_mode,
            ..
        } => arguments
            .iter()
            .map(ScalarArgument::abi_word_count)
            .sum::<usize>()
            .max(failure_mode_max_call_argument_count(failure_mode)),
        Instruction::ReadSlice { failure_mode, .. }
        | Instruction::OpenRead { failure_mode, .. } => {
            failure_mode_max_call_argument_count(failure_mode)
        }
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => max_call_argument_count(then_instructions)
            .max(max_call_argument_count(else_instructions)),
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => max_call_argument_count(condition_instructions)
            .max(max_call_argument_count(body_instructions)),
        Instruction::CheckFailure { failure_mode } => {
            failure_mode_max_call_argument_count(failure_mode)
        }
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::Break
        | Instruction::Continue => 0,
        Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::CloseFd { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::StoreAggregateUsize { .. }
        | Instruction::StoreAggregateI32 { .. }
        | Instruction::StoreAggregateU16 { .. }
        | Instruction::StoreAggregateU32 { .. }
        | Instruction::StoreAggregateU8 { .. }
        | Instruction::StoreAggregateBool { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSlice { .. }
        | Instruction::SetSliceRawParts { .. }
        | Instruction::AddI32 { .. }
        | Instruction::SubtractI32 { .. }
        | Instruction::MultiplyI32 { .. }
        | Instruction::DivideI32 { .. }
        | Instruction::RemainderI32 { .. }
        | Instruction::ShiftLeftI32 { .. }
        | Instruction::ShiftRightI32 { .. }
        | Instruction::AddUsize { .. }
        | Instruction::SubtractUsize { .. }
        | Instruction::MultiplyUsize { .. }
        | Instruction::DivideUsize { .. }
        | Instruction::RemainderUsize { .. }
        | Instruction::ShiftLeftUsize { .. }
        | Instruction::ShiftRightUsize { .. }
        | Instruction::Trap
        | Instruction::Return => 0,
    }
}

fn aggregate_slot_requests(
    instructions: &[Instruction],
) -> Result<Vec<AggregateSlotRequest>, Vec<Diagnostic>> {
    let mut requests = Vec::new();
    record_instruction_list_aggregate_slot_requests(instructions, &mut requests)?;
    Ok(requests)
}

fn record_instruction_list_aggregate_slot_requests(
    instructions: &[Instruction],
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    for instruction in instructions {
        record_instruction_aggregate_slot_requests(instruction, requests)?;
    }

    Ok(())
}

fn record_instruction_aggregate_slot_requests(
    instruction: &Instruction,
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    match instruction {
        Instruction::ReserveAggregateSlot { slot_index, layout } => {
            record_aggregate_slot_request(*slot_index, *layout, requests)
        }
        Instruction::CopyAggregate {
            destination,
            source,
            layout,
        } => {
            if let AggregateLocation::Slot(slot_index) = destination {
                record_aggregate_slot_request(*slot_index, *layout, requests)?;
            }
            if let AggregateLocation::Slot(slot_index) = source {
                record_aggregate_slot_request(*slot_index, *layout, requests)?;
            }
            Ok(())
        }
        Instruction::CopyAggregateRange { .. } => Ok(()),
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            record_instruction_list_aggregate_slot_requests(then_instructions, requests)?;
            record_instruction_list_aggregate_slot_requests(else_instructions, requests)
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            record_instruction_list_aggregate_slot_requests(condition_instructions, requests)?;
            record_instruction_list_aggregate_slot_requests(body_instructions, requests)
        }
        Instruction::CallFallibleI32 { failure_mode, .. }
        | Instruction::CallFallibleU8 { failure_mode, .. }
        | Instruction::CallFallibleUsize { failure_mode, .. }
        | Instruction::CallFallibleBool { failure_mode, .. }
        | Instruction::CallFallibleStr { failure_mode, .. }
        | Instruction::CallFallibleSlice { failure_mode, .. }
        | Instruction::CallFallibleDirectAggregate { failure_mode, .. }
        | Instruction::CallFallibleAggregate { failure_mode, .. }
        | Instruction::CallFallibleVoid { failure_mode, .. }
        | Instruction::ReadSlice { failure_mode, .. }
        | Instruction::OpenRead { failure_mode, .. }
        | Instruction::CheckFailure { failure_mode } => {
            record_failure_mode_aggregate_slot_requests(failure_mode, requests)
        }
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::CloseFd { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSliceRawParts { .. }
        | Instruction::StoreAggregateUsize { .. }
        | Instruction::StoreAggregateI32 { .. }
        | Instruction::StoreAggregateU16 { .. }
        | Instruction::StoreAggregateU32 { .. }
        | Instruction::StoreAggregateU8 { .. }
        | Instruction::StoreAggregateBool { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetSlice { .. }
        | Instruction::AddI32 { .. }
        | Instruction::SubtractI32 { .. }
        | Instruction::MultiplyI32 { .. }
        | Instruction::DivideI32 { .. }
        | Instruction::RemainderI32 { .. }
        | Instruction::ShiftLeftI32 { .. }
        | Instruction::ShiftRightI32 { .. }
        | Instruction::AddUsize { .. }
        | Instruction::SubtractUsize { .. }
        | Instruction::MultiplyUsize { .. }
        | Instruction::DivideUsize { .. }
        | Instruction::RemainderUsize { .. }
        | Instruction::ShiftLeftUsize { .. }
        | Instruction::ShiftRightUsize { .. }
        | Instruction::CallI32 { .. }
        | Instruction::CallU8 { .. }
        | Instruction::CallUsize { .. }
        | Instruction::CallBool { .. }
        | Instruction::CallStr { .. }
        | Instruction::CallSlice { .. }
        | Instruction::CallAggregate { .. }
        | Instruction::CallDirectAggregate { .. }
        | Instruction::CallVoid { .. }
        | Instruction::TailCall { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => Ok(()),
    }
}

fn record_failure_mode_aggregate_slot_requests(
    failure_mode: &FallibleFailureMode,
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => Ok(()),
        FallibleFailureMode::PropagateWithCleanup { instructions, .. } => {
            record_instruction_list_aggregate_slot_requests(instructions, requests)
        }
        FallibleFailureMode::Catch { instructions, .. } => {
            record_instruction_list_aggregate_slot_requests(instructions, requests)
        }
    }
}

fn record_aggregate_slot_request(
    slot_index: usize,
    layout: ValueLayout,
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(existing) = requests
        .iter()
        .find(|request| request.slot_index == slot_index)
    {
        if existing.layout == layout {
            return Ok(());
        }

        return Err(vec![Diagnostic::error(
            "E9005",
            format!("aggregate slot {slot_index} has conflicting ABI layouts"),
        )]);
    }

    requests.push(AggregateSlotRequest::new(slot_index, layout));
    Ok(())
}

fn failure_mode_max_call_argument_count(failure_mode: &FallibleFailureMode) -> usize {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => 0,
        FallibleFailureMode::PropagateWithCleanup { instructions, .. } => {
            max_call_argument_count(instructions)
        }
        FallibleFailureMode::Catch { instructions, .. } => max_call_argument_count(instructions),
    }
}

fn failure_mode_requires_frame(failure_mode: &FallibleFailureMode) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::PropagateWithCleanup { .. } | FallibleFailureMode::Catch { .. } => {
            true
        }
    }
}

fn record_instruction_list_parameter_spill_requests(
    instructions: &[Instruction],
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    for instruction in instructions {
        record_instruction_parameter_spill_requests(
            instruction,
            requests,
            include_value_parameters,
        );
    }
}

fn record_instruction_parameter_spill_requests(
    instruction: &Instruction,
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    match instruction {
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::TailCall { arguments, .. } => {
            record_scalar_arguments_parameter_spill_requests(
                arguments,
                requests,
                include_value_parameters,
            );
        }
        Instruction::CallFallibleI32 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleU8 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleUsize {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleBool {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleStr {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleSlice {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleDirectAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallFallibleVoid {
            arguments,
            failure_mode,
            ..
        } => {
            record_scalar_arguments_parameter_spill_requests(
                arguments,
                requests,
                include_value_parameters,
            );
            record_failure_mode_parameter_spill_requests(
                failure_mode,
                requests,
                include_value_parameters,
            );
        }
        Instruction::If {
            condition,
            then_instructions,
            else_instructions,
            ..
        } => {
            if include_value_parameters {
                record_bool_value_parameter_spill_requests(condition, requests);
            }
            record_instruction_list_parameter_spill_requests(
                then_instructions,
                requests,
                include_value_parameters,
            );
            record_instruction_list_parameter_spill_requests(
                else_instructions,
                requests,
                include_value_parameters,
            );
        }
        Instruction::While {
            condition_instructions,
            condition,
            body_instructions,
            ..
        } => {
            record_instruction_list_parameter_spill_requests(
                condition_instructions,
                requests,
                include_value_parameters,
            );
            if include_value_parameters {
                record_bool_value_parameter_spill_requests(condition, requests);
            }
            record_instruction_list_parameter_spill_requests(
                body_instructions,
                requests,
                include_value_parameters,
            );
        }
        Instruction::CheckFailure { failure_mode } => {
            record_failure_mode_parameter_spill_requests(
                failure_mode,
                requests,
                include_value_parameters,
            );
        }
        Instruction::ReturnFallibleFailure { code, message } => {
            if include_value_parameters {
                record_str_value_parameter_spill_requests(code, requests);
                record_str_value_parameter_spill_requests(message, requests);
            }
        }
        Instruction::ProcessExit { code } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(code, requests);
            }
        }
        Instruction::WriteStr { fd, text } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(fd, requests);
                record_str_value_parameter_spill_requests(text, requests);
            }
        }
        Instruction::WriteSlice { fd, bytes } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(fd, requests);
                record_slice_value_parameter_spill_requests(bytes, requests);
            }
        }
        Instruction::ReadSlice {
            fd,
            buffer,
            failure_mode,
            ..
        } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(fd, requests);
                record_slice_value_parameter_spill_requests(buffer, requests);
            }
            record_failure_mode_parameter_spill_requests(
                failure_mode,
                requests,
                include_value_parameters,
            );
        }
        Instruction::OpenRead {
            path, failure_mode, ..
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(path, requests);
            }
            record_failure_mode_parameter_spill_requests(
                failure_mode,
                requests,
                include_value_parameters,
            );
        }
        Instruction::CloseFd { fd } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(fd, requests);
            }
        }
        Instruction::DarwinSyscall {
            number, arguments, ..
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(number, requests);
                for argument in arguments {
                    record_usize_value_parameter_spill_requests(argument, requests);
                }
            }
        }
        Instruction::CopyStrToPointer {
            pointer,
            offset,
            text,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_str_value_parameter_spill_requests(text, requests);
            }
        }
        Instruction::StoreU8ToPointer {
            pointer,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_u8_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateUsize {
            destination,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    8,
                    requests,
                );
                record_usize_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateI32 {
            destination,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    4,
                    requests,
                );
                record_i32_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateU32 {
            destination,
            offset,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    4,
                    requests,
                );
            }
        }
        Instruction::StoreAggregateU16 {
            destination,
            offset,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    2,
                    requests,
                );
            }
        }
        Instruction::StoreAggregateU8 {
            destination,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    1,
                    requests,
                );
                record_u8_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateBool {
            destination,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *offset,
                    1,
                    requests,
                );
                record_bool_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::LoadAggregateUsize { source, offset, .. }
        | Instruction::LoadAggregateI32 { source, offset, .. }
        | Instruction::LoadAggregateU8 { source, offset, .. }
        | Instruction::LoadAggregateBool { source, offset, .. } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(*source, *offset, 1, requests);
            }
        }
        Instruction::CopyAggregate {
            destination,
            source,
            layout,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    0,
                    layout.size,
                    requests,
                );
                record_aggregate_location_parameter_spill_request(
                    *source,
                    0,
                    layout.size,
                    requests,
                );
            }
        }
        Instruction::CopyAggregateRange {
            destination,
            destination_offset,
            source,
            source_offset,
            layout,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *destination_offset,
                    layout.size,
                    requests,
                );
                record_aggregate_location_parameter_spill_request(
                    *source,
                    *source_offset,
                    layout.size,
                    requests,
                );
            }
        }
        Instruction::SetI32 { value, .. } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::SetU8 { value, .. } => {
            if include_value_parameters {
                record_u8_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::SetUsize { value, .. } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::SetBool { value, .. } => {
            if include_value_parameters {
                record_bool_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::SetStr { value, .. } => {
            if include_value_parameters {
                record_str_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::SetStrRawParts { pointer, len, .. } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(len, requests);
            }
        }
        Instruction::SetSliceRawParts { pointer, len, .. } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(len, requests);
            }
        }
        Instruction::SetSlice { value, .. } => {
            if include_value_parameters {
                record_slice_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::AddI32 { left, right, .. }
        | Instruction::SubtractI32 { left, right, .. }
        | Instruction::MultiplyI32 { left, right, .. }
        | Instruction::DivideI32 { left, right, .. }
        | Instruction::RemainderI32 { left, right, .. }
        | Instruction::ShiftLeftI32 { left, right, .. }
        | Instruction::ShiftRightI32 { left, right, .. } => {
            if include_value_parameters {
                record_i32_value_parameter_spill_requests(left, requests);
                record_i32_value_parameter_spill_requests(right, requests);
            }
        }
        Instruction::AddUsize { left, right, .. }
        | Instruction::SubtractUsize { left, right, .. }
        | Instruction::MultiplyUsize { left, right, .. }
        | Instruction::DivideUsize { left, right, .. }
        | Instruction::RemainderUsize { left, right, .. }
        | Instruction::ShiftLeftUsize { left, right, .. }
        | Instruction::ShiftRightUsize { left, right, .. } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(left, requests);
                record_usize_value_parameter_spill_requests(right, requests);
            }
        }
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => {}
    }
}

fn record_failure_mode_parameter_spill_requests(
    failure_mode: &FallibleFailureMode,
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Catch { instructions, .. } => {
            record_instruction_list_parameter_spill_requests(
                instructions,
                requests,
                include_value_parameters,
            );
        }
    }
}

fn record_scalar_arguments_parameter_spill_requests(
    arguments: &[ScalarArgument],
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    for argument in arguments {
        record_scalar_argument_parameter_spill_requests(
            argument,
            requests,
            include_value_parameters,
        );
    }
}

fn record_scalar_argument_parameter_spill_requests(
    argument: &ScalarArgument,
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    match argument {
        ScalarArgument::Borrow(argument) => {
            record_borrow_source_parameter_spill_request(argument.source, requests);
        }
        ScalarArgument::I32(value) if include_value_parameters => {
            record_i32_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::U8(value) if include_value_parameters => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::Usize(value) if include_value_parameters => {
            record_usize_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::Bool(value) if include_value_parameters => {
            record_bool_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::Str(value) if include_value_parameters => {
            record_str_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::Slice(value) if include_value_parameters => {
            record_slice_value_parameter_spill_requests(value, requests);
        }
        ScalarArgument::AggregateIndirect(_)
        | ScalarArgument::AggregateDirect(_)
        | ScalarArgument::I32(_)
        | ScalarArgument::U8(_)
        | ScalarArgument::Usize(_)
        | ScalarArgument::Bool(_)
        | ScalarArgument::Str(_)
        | ScalarArgument::Slice(_) => {}
    }
}

fn record_i32_value_parameter_spill_requests(value: &I32Value, requests: &mut BTreeSet<usize>) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(I32Location::Parameter(index)) => {
            requests.insert(*index);
        }
        I32Value::Location(I32Location::Return | I32Location::Local(_)) => {}
        I32Value::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
    }
}

fn record_u8_value_parameter_spill_requests(value: &U8Value, requests: &mut BTreeSet<usize>) {
    match value {
        U8Value::Const(_) => {}
        U8Value::Location(U8Location::Parameter(index)) => {
            requests.insert(*index);
        }
        U8Value::Location(U8Location::Return | U8Location::Local(_)) => {}
        U8Value::StrIndex { source, index } => {
            record_str_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        U8Value::StaticStrIndex { index, .. } => {
            record_usize_value_parameter_spill_requests(index, requests);
        }
        U8Value::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

fn record_usize_value_parameter_spill_requests(value: &UsizeValue, requests: &mut BTreeSet<usize>) {
    match value {
        UsizeValue::Const(_) => {}
        UsizeValue::Location(UsizeLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        UsizeValue::Location(UsizeLocation::Return | UsizeLocation::Local(_)) => {}
        UsizeValue::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        UsizeValue::StrLen(StrLocation::Parameter(index))
        | UsizeValue::SliceLen(SliceLocation::Parameter(index)) => {
            if let Some(len_index) = index.checked_add(1) {
                requests.insert(len_index);
            }
        }
        UsizeValue::StrLen(StrLocation::Return | StrLocation::Local(_))
        | UsizeValue::SliceLen(SliceLocation::Return | SliceLocation::Local(_)) => {}
    }
}

fn record_bool_value_parameter_spill_requests(value: &BoolValue, requests: &mut BTreeSet<usize>) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(BoolLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        BoolValue::Location(BoolLocation::Return | BoolLocation::Local(_)) => {}
        BoolValue::Not(value) => {
            record_bool_value_parameter_spill_requests(value, requests);
        }
        BoolValue::Logical { left, right, .. } | BoolValue::BoolComparison { left, right, .. } => {
            record_bool_value_parameter_spill_requests(left, requests);
            record_bool_value_parameter_spill_requests(right, requests);
        }
        BoolValue::I32Comparison { left, right, .. } => {
            record_i32_value_parameter_spill_requests(left, requests);
            record_i32_value_parameter_spill_requests(right, requests);
        }
        BoolValue::UsizeComparison { left, right, .. } => {
            record_usize_value_parameter_spill_requests(left, requests);
            record_usize_value_parameter_spill_requests(right, requests);
        }
    }
}

fn record_str_value_parameter_spill_requests(value: &StrValue, requests: &mut BTreeSet<usize>) {
    match value {
        StrValue::StaticBytes(_) => {}
        StrValue::Location(location) => {
            record_str_location_parameter_pair_spill_requests(*location, requests);
        }
    }
}

fn record_slice_value_parameter_spill_requests(value: &SliceValue, requests: &mut BTreeSet<usize>) {
    match value {
        SliceValue::StrBytes(text) => {
            record_str_value_parameter_spill_requests(text, requests);
        }
        SliceValue::Location(location) => {
            record_slice_location_parameter_pair_spill_requests(*location, requests);
        }
    }
}

fn record_str_location_parameter_pair_spill_requests(
    location: StrLocation,
    requests: &mut BTreeSet<usize>,
) {
    if let StrLocation::Parameter(index) = location {
        requests.insert(index);
        if let Some(len_index) = index.checked_add(1) {
            requests.insert(len_index);
        }
    }
}

fn record_slice_location_parameter_pair_spill_requests(
    location: SliceLocation,
    requests: &mut BTreeSet<usize>,
) {
    if let SliceLocation::Parameter(index) = location {
        requests.insert(index);
        if let Some(len_index) = index.checked_add(1) {
            requests.insert(len_index);
        }
    }
}

fn record_aggregate_location_parameter_spill_request(
    location: AggregateLocation,
    offset: u32,
    size: u64,
    requests: &mut BTreeSet<usize>,
) {
    match location {
        AggregateLocation::Parameter(index) => {
            requests.insert(index);
        }
        AggregateLocation::DirectParameter { start_index } => {
            let offset = u64::from(offset);
            let Some(last_byte_offset) = size
                .checked_sub(1)
                .and_then(|last| offset.checked_add(last))
            else {
                return;
            };
            let first_word = offset / 8;
            let last_word = last_byte_offset / 8;
            for word in first_word..=last_word {
                if let Some(parameter_index) = usize::try_from(word)
                    .ok()
                    .and_then(|word| start_index.checked_add(word))
                {
                    requests.insert(parameter_index);
                }
            }
        }
        AggregateLocation::Return
        | AggregateLocation::DirectReturn
        | AggregateLocation::Slot(_) => {}
    }
}

fn record_borrow_source_parameter_spill_request(
    source: BorrowSource,
    requests: &mut BTreeSet<usize>,
) {
    match source {
        BorrowSource::I32(I32Location::Parameter(index))
        | BorrowSource::U8(U8Location::Parameter(index))
        | BorrowSource::Usize(UsizeLocation::Parameter(index))
        | BorrowSource::Bool(BoolLocation::Parameter(index)) => {
            requests.insert(index);
        }
        BorrowSource::AggregateParameter(index)
        | BorrowSource::AggregateParameterField {
            parameter_index: index,
            ..
        } => {
            requests.insert(index);
        }
        BorrowSource::I32(I32Location::Return | I32Location::Local(_))
        | BorrowSource::U8(U8Location::Return | U8Location::Local(_))
        | BorrowSource::Usize(UsizeLocation::Return | UsizeLocation::Local(_))
        | BorrowSource::Bool(BoolLocation::Return | BoolLocation::Local(_))
        | BorrowSource::AggregateSlot(_)
        | BorrowSource::AggregateSlotField { .. } => {}
    }
}

fn record_instruction_list_scalar_locals(
    instructions: &[Instruction],
    highest_local_index: &mut Option<usize>,
) {
    for instruction in instructions {
        record_instruction_scalar_locals(instruction, highest_local_index);
    }
}

fn record_instruction_scalar_locals(
    instruction: &Instruction,
    highest_local_index: &mut Option<usize>,
) {
    match instruction {
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => {}
        Instruction::CheckFailure { failure_mode } => {
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::ReturnFallibleFailure { code, message } => {
            record_str_value(code, highest_local_index);
            record_str_value(message, highest_local_index);
        }
        Instruction::ProcessExit { code } => {
            record_i32_value(code, highest_local_index);
        }
        Instruction::StoreAggregateUsize { value, .. } => {
            record_usize_value(value, highest_local_index);
        }
        Instruction::StoreAggregateI32 { value, .. } => {
            record_i32_value(value, highest_local_index);
        }
        Instruction::StoreAggregateU16 { .. } => {}
        Instruction::StoreAggregateU32 { .. } => {}
        Instruction::StoreAggregateU8 { value, .. } => {
            record_u8_value(value, highest_local_index);
        }
        Instruction::StoreAggregateBool { value, .. } => {
            record_bool_value(value, highest_local_index);
        }
        Instruction::LoadAggregateUsize { destination, .. } => {
            record_usize_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateI32 { destination, .. } => {
            record_i32_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateU8 { destination, .. } => {
            record_u8_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateBool { destination, .. } => {
            record_bool_location(*destination, highest_local_index);
        }
        Instruction::WriteStr { fd, text } => {
            record_i32_value(fd, highest_local_index);
            record_str_value(text, highest_local_index);
        }
        Instruction::WriteSlice { fd, bytes } => {
            record_i32_value(fd, highest_local_index);
            record_slice_value(bytes, highest_local_index);
        }
        Instruction::ReadSlice {
            destination,
            fd,
            buffer,
            failure_mode,
        } => {
            record_usize_location(*destination, highest_local_index);
            record_i32_value(fd, highest_local_index);
            record_slice_value(buffer, highest_local_index);
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::OpenRead {
            destination,
            path,
            failure_mode,
        } => {
            record_i32_location(*destination, highest_local_index);
            record_usize_value(path, highest_local_index);
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CloseFd { fd } => {
            record_i32_value(fd, highest_local_index);
        }
        Instruction::DarwinSyscall {
            number, arguments, ..
        } => {
            record_usize_value(number, highest_local_index);
            for argument in arguments {
                record_usize_value(argument, highest_local_index);
            }
        }
        Instruction::CopyStrToPointer {
            pointer,
            offset,
            text,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_str_value(text, highest_local_index);
        }
        Instruction::StoreU8ToPointer {
            pointer,
            offset,
            value,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_u8_value(value, highest_local_index);
        }
        Instruction::TailCall { arguments, .. } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::SetI32 { destination, value } => {
            record_i32_location(*destination, highest_local_index);
            record_i32_value(value, highest_local_index);
        }
        Instruction::SetU8 { destination, value } => {
            record_u8_location(*destination, highest_local_index);
            record_u8_value(value, highest_local_index);
        }
        Instruction::SetUsize { destination, value } => {
            record_usize_location(*destination, highest_local_index);
            record_usize_value(value, highest_local_index);
        }
        Instruction::SetBool { destination, value } => {
            record_bool_location(*destination, highest_local_index);
            record_bool_value(value, highest_local_index);
        }
        Instruction::SetStr { destination, value } => {
            record_str_location(*destination, highest_local_index);
            record_str_value(value, highest_local_index);
        }
        Instruction::SetStrRawParts {
            destination,
            pointer,
            len,
        } => {
            record_str_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(len, highest_local_index);
        }
        Instruction::SetSliceRawParts {
            destination,
            pointer,
            len,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(len, highest_local_index);
        }
        Instruction::SetSlice { destination, value } => {
            record_slice_location(*destination, highest_local_index);
            record_slice_value(value, highest_local_index);
        }
        Instruction::AddI32 {
            destination,
            left,
            right,
        }
        | Instruction::SubtractI32 {
            destination,
            left,
            right,
        }
        | Instruction::MultiplyI32 {
            destination,
            left,
            right,
        }
        | Instruction::DivideI32 {
            destination,
            left,
            right,
        }
        | Instruction::RemainderI32 {
            destination,
            left,
            right,
        }
        | Instruction::ShiftLeftI32 {
            destination,
            left,
            right,
        }
        | Instruction::ShiftRightI32 {
            destination,
            left,
            right,
        } => {
            record_i32_location(*destination, highest_local_index);
            record_i32_value(left, highest_local_index);
            record_i32_value(right, highest_local_index);
        }
        Instruction::AddUsize {
            destination,
            left,
            right,
        }
        | Instruction::SubtractUsize {
            destination,
            left,
            right,
        }
        | Instruction::MultiplyUsize {
            destination,
            left,
            right,
        }
        | Instruction::DivideUsize {
            destination,
            left,
            right,
        }
        | Instruction::RemainderUsize {
            destination,
            left,
            right,
        }
        | Instruction::ShiftLeftUsize {
            destination,
            left,
            right,
        }
        | Instruction::ShiftRightUsize {
            destination,
            left,
            right,
        } => {
            record_usize_location(*destination, highest_local_index);
            record_usize_value(left, highest_local_index);
            record_usize_value(right, highest_local_index);
        }
        Instruction::CallI32 {
            destination,
            arguments,
            ..
        } => {
            record_i32_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleI32 {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_i32_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallU8 {
            destination,
            arguments,
            ..
        } => {
            record_u8_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleU8 {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_u8_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallUsize {
            destination,
            arguments,
            ..
        } => {
            record_usize_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleUsize {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_usize_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallBool {
            destination,
            arguments,
            ..
        } => {
            record_bool_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleBool {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_bool_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallStr {
            destination,
            arguments,
            ..
        } => {
            record_str_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleStr {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_str_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallSlice {
            destination,
            arguments,
            ..
        } => {
            record_slice_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleSlice {
            destination,
            arguments,
            failure_mode,
            ..
        } => {
            record_slice_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleDirectAggregate {
            arguments,
            failure_mode,
            ..
        } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallFallibleAggregate {
            arguments,
            failure_mode,
            ..
        } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallVoid { arguments, .. } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallFallibleVoid {
            arguments,
            failure_mode,
            ..
        } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::If {
            condition,
            then_instructions,
            else_instructions,
        } => {
            record_bool_value(condition, highest_local_index);
            record_instruction_list_scalar_locals(then_instructions, highest_local_index);
            record_instruction_list_scalar_locals(else_instructions, highest_local_index);
        }
        Instruction::While {
            condition_instructions,
            condition,
            body_instructions,
        } => {
            record_instruction_list_scalar_locals(condition_instructions, highest_local_index);
            record_bool_value(condition, highest_local_index);
            record_instruction_list_scalar_locals(body_instructions, highest_local_index);
        }
    }
}

fn record_failure_mode_scalar_locals(
    failure_mode: &FallibleFailureMode,
    highest_local_index: &mut Option<usize>,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup {
            code,
            message,
            instructions,
        } => {
            record_str_location(*code, highest_local_index);
            record_str_location(*message, highest_local_index);
            record_instruction_list_scalar_locals(instructions, highest_local_index);
        }
        FallibleFailureMode::Catch {
            code,
            message,
            instructions,
        } => {
            record_str_location(*code, highest_local_index);
            record_str_location(*message, highest_local_index);
            record_instruction_list_scalar_locals(instructions, highest_local_index);
        }
    }
}

fn record_i32_value(value: &I32Value, highest_local_index: &mut Option<usize>) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(location) => record_i32_location(*location, highest_local_index),
        I32Value::U8ZeroExtend(value) => record_u8_value(value, highest_local_index),
    }
}

fn record_scalar_argument(argument: &ScalarArgument, highest_local_index: &mut Option<usize>) {
    match argument {
        ScalarArgument::I32(value) => record_i32_value(value, highest_local_index),
        ScalarArgument::U8(value) => record_u8_value(value, highest_local_index),
        ScalarArgument::Usize(value) => record_usize_value(value, highest_local_index),
        ScalarArgument::Bool(value) => record_bool_value(value, highest_local_index),
        ScalarArgument::Str(value) => record_str_value(value, highest_local_index),
        ScalarArgument::Slice(value) => record_slice_value(value, highest_local_index),
        ScalarArgument::Borrow(argument) => {
            record_borrow_source(argument.source, highest_local_index);
        }
        ScalarArgument::AggregateIndirect(_) | ScalarArgument::AggregateDirect(_) => {}
    }
}

fn record_borrow_source(source: BorrowSource, highest_local_index: &mut Option<usize>) {
    match source {
        BorrowSource::I32(location) => record_i32_location(location, highest_local_index),
        BorrowSource::U8(location) => record_u8_location(location, highest_local_index),
        BorrowSource::Usize(location) => record_usize_location(location, highest_local_index),
        BorrowSource::Bool(location) => record_bool_location(location, highest_local_index),
        BorrowSource::AggregateSlot(_)
        | BorrowSource::AggregateSlotField { .. }
        | BorrowSource::AggregateParameter(_)
        | BorrowSource::AggregateParameterField { .. } => {}
    }
}

fn record_i32_location(location: I32Location, highest_local_index: &mut Option<usize>) {
    if let I32Location::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_u8_value(value: &U8Value, highest_local_index: &mut Option<usize>) {
    match value {
        U8Value::Const(_) => {}
        U8Value::Location(location) => record_u8_location(*location, highest_local_index),
        U8Value::StrIndex { source, index } => {
            record_str_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
        U8Value::StaticStrIndex { index, .. } => {
            record_usize_value(index, highest_local_index);
        }
        U8Value::SliceIndex { source, index } => {
            record_slice_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
    }
}

fn record_u8_location(location: U8Location, highest_local_index: &mut Option<usize>) {
    if let U8Location::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_usize_value(value: &UsizeValue, highest_local_index: &mut Option<usize>) {
    match value {
        UsizeValue::Const(_) => {}
        UsizeValue::Location(location) => record_usize_location(*location, highest_local_index),
        UsizeValue::U8ZeroExtend(value) => record_u8_value(value, highest_local_index),
        UsizeValue::StrLen(location) => record_str_location(*location, highest_local_index),
        UsizeValue::SliceLen(location) => record_slice_location(*location, highest_local_index),
    }
}

fn record_usize_location(location: UsizeLocation, highest_local_index: &mut Option<usize>) {
    if let UsizeLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_bool_value(value: &BoolValue, highest_local_index: &mut Option<usize>) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(location) => record_bool_location(*location, highest_local_index),
        BoolValue::Not(inner) => record_bool_value(inner, highest_local_index),
        BoolValue::Logical { left, right, .. } => {
            record_bool_value(left, highest_local_index);
            record_bool_value(right, highest_local_index);
        }
        BoolValue::I32Comparison { left, right, .. } => {
            record_i32_value(left, highest_local_index);
            record_i32_value(right, highest_local_index);
        }
        BoolValue::UsizeComparison { left, right, .. } => {
            record_usize_value(left, highest_local_index);
            record_usize_value(right, highest_local_index);
        }
        BoolValue::BoolComparison { left, right, .. } => {
            record_bool_value(left, highest_local_index);
            record_bool_value(right, highest_local_index);
        }
    }
}

fn record_bool_location(location: BoolLocation, highest_local_index: &mut Option<usize>) {
    if let BoolLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_str_value(value: &StrValue, highest_local_index: &mut Option<usize>) {
    match value {
        StrValue::StaticBytes(_) => {}
        StrValue::Location(location) => record_str_location(*location, highest_local_index),
    }
}

fn record_str_location(location: StrLocation, highest_local_index: &mut Option<usize>) {
    if let StrLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
        record_scalar_local(index + 1, highest_local_index);
    }
}

fn record_slice_value(value: &SliceValue, highest_local_index: &mut Option<usize>) {
    match value {
        SliceValue::StrBytes(text) => record_str_value(text, highest_local_index),
        SliceValue::Location(location) => record_slice_location(*location, highest_local_index),
    }
}

fn record_slice_location(location: SliceLocation, highest_local_index: &mut Option<usize>) {
    if let SliceLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
        record_scalar_local(index + 1, highest_local_index);
    }
}

fn record_scalar_local(index: usize, highest_local_index: &mut Option<usize>) {
    *highest_local_index = Some(highest_local_index.map_or(index, |highest| highest.max(index)));
}

fn aggregate_slots(
    requests: &[AggregateSlotRequest],
    base_offset: usize,
) -> Result<(Vec<AggregateSlot>, usize), Vec<Diagnostic>> {
    let mut slots = Vec::with_capacity(requests.len());
    let mut next_offset = base_offset;

    for request in requests {
        let size = usize::try_from(request.layout.size).map_err(|_| {
            frame_too_large_diagnostic("aggregate slot size exceeds host usize range")
        })?;
        let align = usize::try_from(request.layout.align).map_err(|_| {
            frame_too_large_diagnostic("aggregate slot alignment exceeds host usize range")
        })?;
        if align == 0 || !align.is_power_of_two() || align > STACK_ALIGNMENT {
            return Err(frame_too_large_diagnostic(
                "aggregate slot alignment is not supported by backend v0",
            ));
        }

        let offset = align_usize(next_offset, align);
        if offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
            return Err(frame_too_large_diagnostic(
                "aggregate slot offset exceeds ARM64 x-register load/store immediate range",
            ));
        }
        if size > 0 {
            let end_offset = offset.checked_add(size - 1).ok_or_else(|| {
                frame_too_large_diagnostic("aggregate slot end overflows host usize")
            })?;
            if end_offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "aggregate slot end exceeds ARM64 x-register load/store immediate range",
                ));
            }
        }

        slots.push(AggregateSlot {
            slot_index: request.slot_index,
            offset: offset as u32,
            size: size as u32,
            align: align as u32,
        });
        next_offset = offset.checked_add(size).ok_or_else(|| {
            frame_too_large_diagnostic("aggregate slot offset overflows host usize")
        })?;
    }

    Ok((slots, next_offset - base_offset))
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn frame_too_large_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("function stack frame is too large for backend v0: {reason}"),
    )]
}

const STACK_ALIGNMENT: usize = 16;
const SCALAR_SLOT_SIZE: usize = 8;
const SAVED_X30_SLOT_SIZE: usize = 8;
const ADD_SUB_SP_IMM_MAX: u32 = 0x00ff_f000;
const LDR_STR_W_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 4;
const LDR_STR_X_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        AggregateArgument, AggregateArgumentSource, BoolComparisonOperator, BorrowArgument,
        CallTarget, DirectAggregateArgument, FallibleFailureMode, ScalarArgument, SliceLocation,
        SliceValue, StrValue, Type,
    };

    #[test]
    fn plans_current_ir_functions_as_frameless() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("answer"),
                arguments: vec![],
            }],
        };

        assert_eq!(
            plan_function_frame(&function).unwrap(),
            FunctionFrame::Frameless
        );
    }

    #[test]
    fn computes_aligned_frame_with_saved_x30_only() {
        let layout = FrameLayout::for_slot_counts(0, 0).unwrap();

        assert_eq!(layout.frame_size(), 16);
        assert_eq!(layout.saved_x30_offset(), 8);
        assert!(layout.scalar_spill_slots().is_empty());
        assert!(layout.argument_staging_slots().is_empty());
    }

    #[test]
    fn computes_scalar_spill_slots_below_saved_x30() {
        let layout = FrameLayout::for_slot_counts(3, 0).unwrap();

        assert_eq!(layout.frame_size(), 32);
        assert_eq!(layout.saved_x30_offset(), 24);
        assert_eq!(
            layout.scalar_spill_slots(),
            &[
                ScalarSpillSlot {
                    local_index: 0,
                    offset: 0
                },
                ScalarSpillSlot {
                    local_index: 1,
                    offset: 8
                },
                ScalarSpillSlot {
                    local_index: 2,
                    offset: 16
                },
            ]
        );
    }

    #[test]
    fn computes_argument_staging_slots_above_scalar_spills() {
        let layout = FrameLayout::for_slot_counts(2, 3).unwrap();

        assert_eq!(layout.frame_size(), 48);
        assert_eq!(layout.saved_x30_offset(), 40);
        assert_eq!(
            layout.argument_staging_slots(),
            &[
                ArgumentStagingSlot {
                    abi_word_index: 0,
                    offset: 16
                },
                ArgumentStagingSlot {
                    abi_word_index: 1,
                    offset: 24
                },
                ArgumentStagingSlot {
                    abi_word_index: 2,
                    offset: 32
                },
            ]
        );
    }

    #[test]
    fn computes_parameter_spill_slots_below_scalar_and_argument_slots() {
        let layout = FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
            2,
            1,
            &[8, 0, 8],
            &[],
            false,
        )
        .unwrap();

        assert_eq!(layout.frame_size(), 48);
        assert_eq!(layout.saved_x30_offset(), 40);
        assert_eq!(
            layout.parameter_spill_slots(),
            &[
                ParameterSpillSlot {
                    parameter_index: 0,
                    offset: 0
                },
                ParameterSpillSlot {
                    parameter_index: 8,
                    offset: 8
                },
            ]
        );
        assert_eq!(
            layout.scalar_spill_slots(),
            &[
                ScalarSpillSlot {
                    local_index: 0,
                    offset: 16
                },
                ScalarSpillSlot {
                    local_index: 1,
                    offset: 24
                },
            ]
        );
        assert_eq!(
            layout.argument_staging_slots(),
            &[ArgumentStagingSlot {
                abi_word_index: 0,
                offset: 32
            }]
        );
    }

    #[test]
    fn computes_aggregate_slots_above_argument_staging_with_alignment() {
        let layout = FrameLayout::for_slot_counts_with_aggregate_slots(
            1,
            1,
            &[
                AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
                AggregateSlotRequest::new(1, ValueLayout::new(16, 16)),
            ],
        )
        .unwrap();

        assert_eq!(layout.frame_size(), 80);
        assert_eq!(layout.saved_x30_offset(), 72);
        assert_eq!(
            layout.aggregate_slots(),
            &[
                AggregateSlot {
                    slot_index: 0,
                    offset: 16,
                    size: 24,
                    align: 8,
                },
                AggregateSlot {
                    slot_index: 1,
                    offset: 48,
                    size: 16,
                    align: 16,
                },
            ]
        );
        assert_eq!(layout.aggregate_slot(1).unwrap().offset(), 48);
    }

    #[test]
    fn rejects_unsupported_aggregate_slot_alignment() {
        let error = FrameLayout::for_slot_counts_with_aggregate_slots(
            0,
            0,
            &[AggregateSlotRequest::new(0, ValueLayout::new(8, 32))],
        )
        .unwrap_err();

        assert_eq!(error[0].code, "E9005");
    }

    #[test]
    fn plans_aggregate_slot_requests_from_ir_instructions() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();
        let FunctionFrame::Framed(layout) = frame else {
            panic!("aggregate slot reservation should require a frame");
        };

        assert_eq!(layout.frame_size(), 32);
        assert_eq!(layout.saved_x30_offset(), 24);
        assert_eq!(
            layout.aggregate_slots(),
            &[AggregateSlot {
                slot_index: 0,
                offset: 0,
                size: 24,
                align: 8,
            }]
        );
    }

    #[test]
    fn deduplicates_aggregate_slot_requests_from_nested_control_flow() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::Void)),
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    }],
                    else_instructions: vec![Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    }],
                },
                Instruction::CheckFailure {
                    failure_mode: FallibleFailureMode::Catch {
                        code: StrLocation::Local(0),
                        message: StrLocation::Local(2),
                        instructions: vec![Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout: ValueLayout::new(16, 16),
                        }],
                    },
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();
        let FunctionFrame::Framed(layout) = frame else {
            panic!("aggregate slot reservation should require a frame");
        };

        assert_eq!(
            layout.aggregate_slots(),
            &[
                AggregateSlot {
                    slot_index: 0,
                    offset: 32,
                    size: 24,
                    align: 8,
                },
                AggregateSlot {
                    slot_index: 1,
                    offset: 64,
                    size: 16,
                    align: 16,
                },
            ]
        );
    }

    #[test]
    fn plans_frame_slots_from_while_condition_and_body() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::While {
                    condition_instructions: vec![Instruction::CallBool {
                        destination: BoolLocation::Local(2),
                        target: CallTarget::same_file("ready"),
                        arguments: vec![],
                    }],
                    condition: BoolValue::Location(BoolLocation::Local(2)),
                    body_instructions: vec![Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(8, 8),
                    }],
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    3,
                    0,
                    &[AggregateSlotRequest::new(0, ValueLayout::new(8, 8))]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_call_requires_frame_and_counts_argument_slots() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                        1,
                    )))],
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    2,
                    1,
                    &[AggregateSlotRequest::new(0, ValueLayout::new(24, 8))]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_value_arguments_count_abi_staging_slots() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        ScalarArgument::AggregateIndirect(AggregateArgument {
                            source: AggregateArgumentSource::Slot(0),
                        }),
                        ScalarArgument::AggregateDirect(DirectAggregateArgument {
                            source: AggregateArgumentSource::Slot(1),
                            layout: ValueLayout::new(16, 8),
                            words: 2,
                        }),
                    ],
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    0,
                    3,
                    &[
                        AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
                        AggregateSlotRequest::new(1, ValueLayout::new(16, 8)),
                    ]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_return_store_does_not_require_frame() {
        let function = Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 8,
                    value: UsizeValue::Const(3),
                },
                Instruction::Return,
            ],
        };

        assert_eq!(
            plan_function_frame(&function).unwrap(),
            FunctionFrame::Frameless
        );
    }

    #[test]
    fn aggregate_slot_store_requires_frame_and_counts_value_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: UsizeValue::Location(UsizeLocation::Local(1)),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    2,
                    0,
                    &[AggregateSlotRequest::new(0, ValueLayout::new(24, 8))]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_slot_load_requires_frame_and_counts_destination_local() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(2),
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    3,
                    0,
                    &[AggregateSlotRequest::new(0, ValueLayout::new(16, 8))]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_copy_requires_frame_and_reserves_slots() {
        let function = Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    0,
                    &[],
                    &[
                        AggregateSlotRequest::new(1, ValueLayout::new(24, 8)),
                        AggregateSlotRequest::new(0, ValueLayout::new(24, 8)),
                    ],
                    true,
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_range_copy_uses_explicit_slot_reservations() {
        let function = Function {
            name: "copy_header".to_string(),
            target: crate::ir::CallTarget::same_file("copy_header".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_aggregate_slots(
                    0,
                    0,
                    &[
                        AggregateSlotRequest::new(0, ValueLayout::new(32, 8)),
                        AggregateSlotRequest::new(1, ValueLayout::new(16, 8)),
                    ]
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_range_copy_from_direct_parameter_after_call_reserves_parameter_spill_slots() {
        let function = Function {
            name: "identity".to_string(),
            target: crate::ir::CallTarget::same_file("identity".to_string()),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(9, 1),
                words: 2,
            },
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 0,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    source_offset: 0,
                    layout: ValueLayout::new(9, 1),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    0,
                    &[8, 9],
                    &[],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn aggregate_range_copy_to_borrowed_parameter_after_call_reserves_parameter_spill_slot() {
        let function = Function {
            name: "set_header".to_string(),
            target: crate::ir::CallTarget::same_file("set_header".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Parameter(0),
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    0,
                    &[0],
                    &[AggregateSlotRequest::new(0, ValueLayout::new(16, 8))],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn counts_scalar_slots_from_nested_i32_and_bool_locals() {
        let instructions = vec![Instruction::If {
            condition: BoolValue::BoolComparison {
                operator: BoolComparisonOperator::Equal,
                left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                right: Box::new(BoolValue::Const(true)),
            },
            then_instructions: vec![Instruction::AddI32 {
                destination: I32Location::Local(3),
                left: I32Value::Location(I32Location::Local(0)),
                right: I32Value::Const(1),
            }],
            else_instructions: vec![Instruction::SetBool {
                destination: BoolLocation::Local(2),
                value: BoolValue::Const(false),
            }],
        }];

        assert_eq!(scalar_spill_slot_count(&instructions), 4);
    }

    #[test]
    fn call_i32_requires_frame_and_counts_destination_and_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallI32 {
                destination: I32Location::Local(2),
                target: CallTarget::same_file("answer"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
        );
    }

    #[test]
    fn call_with_scalar_parameter_borrow_reserves_parameter_spill_slot() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("inspect"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::I32(I32Location::Parameter(8)),
                })],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    1,
                    &[8],
                    &[],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn store_to_borrowed_aggregate_parameter_after_call_reserves_parameter_spill_slot() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Parameter(0),
                    offset: 4,
                    value: I32Value::Const(99),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    0,
                    &[0],
                    &[],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn direct_aggregate_parameter_field_load_after_call_reserves_parameter_spill_slot() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 0,
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    0,
                    0,
                    &[0],
                    &[],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn normal_call_function_spills_parameter_values_used_later() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Parameter(0)),
                },
                Instruction::Return,
            ],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(
                FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
                    1,
                    0,
                    &[0],
                    &[],
                    false
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn call_bool_requires_frame_and_counts_destination_and_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallBool {
                destination: BoolLocation::Local(2),
                target: CallTarget::same_file("ready"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
        );
    }

    #[test]
    fn call_fallible_i32_requires_frame_and_counts_destination_and_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::CallFallibleI32 {
                destination: I32Location::Local(2),
                target: CallTarget::same_file("answer"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
                failure_mode: FallibleFailureMode::Propagate,
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
        );
    }

    #[test]
    fn call_void_requires_frame_and_counts_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallVoid {
                target: CallTarget::same_file("effect"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(2, 1).unwrap())
        );
    }

    #[test]
    fn call_fallible_void_requires_frame_and_counts_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::Void)),
            instructions: vec![Instruction::CallFallibleVoid {
                target: CallTarget::same_file("effect"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    1,
                )))],
                failure_mode: FallibleFailureMode::Propagate,
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(2, 1).unwrap())
        );
    }

    #[test]
    fn tail_call_with_arguments_requires_frame_and_argument_staging_slots() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("answer"),
                arguments: vec![
                    ScalarArgument::I32(I32Value::Const(40)),
                    ScalarArgument::I32(I32Value::Const(2)),
                ],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 2).unwrap())
        );
    }

    #[test]
    fn tail_call_with_str_argument_counts_two_argument_staging_slots() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("answer"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::StaticBytes(b"Nocter".to_vec())),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 3).unwrap())
        );
    }

    #[test]
    fn tail_call_with_slice_argument_counts_two_argument_staging_slots() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("answer"),
                arguments: vec![
                    ScalarArgument::Slice(SliceValue::Location(SliceLocation::Parameter(0))),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 3).unwrap())
        );
    }

    #[test]
    fn tail_call_with_local_argument_counts_argument_local() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("answer"),
                arguments: vec![ScalarArgument::I32(I32Value::Location(I32Location::Local(
                    2,
                )))],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_slot_counts(3, 1).unwrap())
        );
    }

    #[test]
    fn rejects_frame_when_w_spill_offset_is_not_encodable() {
        let error = FrameLayout::for_slot_counts(4097, 0).unwrap_err();

        assert_eq!(error[0].code, "E9005");
    }
}
