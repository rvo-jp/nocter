use crate::abi::ValueLayout;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, BoolValue, BorrowSource, FallibleFailureMode, Function, I32Location, I32Value,
    Instruction, ScalarArgument, SliceLocation, SliceValue, StrLocation, StrValue, U8Location,
    U8Value, UsizeLocation, UsizeValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FunctionFrame {
    Frameless,
    Framed(FrameLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrameLayout {
    frame_size: u32,
    saved_x30_offset: u32,
    scalar_spill_slots: Vec<ScalarSpillSlot>,
    argument_staging_slots: Vec<ArgumentStagingSlot>,
    aggregate_slots: Vec<AggregateSlot>,
}

impl FrameLayout {
    pub(super) fn frame_size(&self) -> u32 {
        self.frame_size
    }

    pub(super) fn saved_x30_offset(&self) -> u32 {
        self.saved_x30_offset
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

    pub(super) fn for_slot_counts(
        scalar_spill_count: usize,
        argument_staging_count: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::for_slot_counts_with_aggregate_slots(scalar_spill_count, argument_staging_count, &[])
    }

    #[allow(dead_code)]
    pub(super) fn for_slot_counts_with_aggregate_slots(
        scalar_spill_count: usize,
        argument_staging_count: usize,
        aggregate_slot_requests: &[AggregateSlotRequest],
    ) -> Result<Self, Vec<Diagnostic>> {
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
        let scalar_slot_bytes = scalar_spill_bytes
            .checked_add(argument_staging_bytes)
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

        let mut scalar_spill_slots = Vec::with_capacity(scalar_spill_count);
        for local_index in 0..scalar_spill_count {
            let offset = local_index * SCALAR_SLOT_SIZE;
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
            let offset = scalar_spill_bytes
                .checked_add(argument_index * SCALAR_SLOT_SIZE)
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
            scalar_spill_slots,
            argument_staging_slots,
            aggregate_slots,
        })
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
    if !function_requires_frame(&function.instructions) {
        return Ok(FunctionFrame::Frameless);
    }

    FrameLayout::for_slot_counts(
        scalar_spill_slot_count(&function.instructions),
        max_call_argument_count(&function.instructions),
    )
    .map(FunctionFrame::Framed)
}

fn function_requires_frame(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_requires_frame)
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
        | Instruction::CallVoid { .. }
        | Instruction::CallFallibleVoid { .. }
        | Instruction::WriteStr { .. } => true,
        Instruction::TailCall { arguments, .. } => !arguments.is_empty(),
        Instruction::CheckFailure { failure_mode } => failure_mode_requires_frame(failure_mode),
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::SetI32 { .. }
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
        | Instruction::Trap
        | Instruction::Return => false,
    }
}

fn scalar_spill_slot_count(instructions: &[Instruction]) -> usize {
    let mut highest_local_index = None;
    record_instruction_list_scalar_locals(instructions, &mut highest_local_index);
    highest_local_index.map_or(0, |index| index + 1)
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
        | Instruction::CallVoid { arguments, .. }
        | Instruction::TailCall { arguments, .. } => {
            arguments.iter().map(ScalarArgument::abi_word_count).sum()
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
        | Instruction::CallFallibleVoid {
            arguments,
            failure_mode,
            ..
        } => arguments
            .iter()
            .map(ScalarArgument::abi_word_count)
            .sum::<usize>()
            .max(failure_mode_max_call_argument_count(failure_mode)),
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => max_call_argument_count(then_instructions)
            .max(max_call_argument_count(else_instructions)),
        Instruction::CheckFailure { failure_mode } => {
            failure_mode_max_call_argument_count(failure_mode)
        }
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnFallibleFailure { .. } => 0,
        Instruction::WriteStr { .. }
        | Instruction::SetI32 { .. }
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
        | Instruction::Trap
        | Instruction::Return => 0,
    }
}

fn failure_mode_max_call_argument_count(failure_mode: &FallibleFailureMode) -> usize {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => 0,
        FallibleFailureMode::Catch { instructions, .. } => max_call_argument_count(instructions),
    }
}

fn failure_mode_requires_frame(failure_mode: &FallibleFailureMode) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::Catch { .. } => true,
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
        | Instruction::Trap
        | Instruction::Return => {}
        Instruction::CheckFailure { failure_mode } => {
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::ReturnFallibleFailure { code, message } => {
            record_str_value(code, highest_local_index);
            record_str_value(message, highest_local_index);
        }
        Instruction::WriteStr { fd, text } => {
            record_i32_value(fd, highest_local_index);
            record_str_value(text, highest_local_index);
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
    }
}

fn record_failure_mode_scalar_locals(
    failure_mode: &FallibleFailureMode,
    highest_local_index: &mut Option<usize>,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
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
    }
}

fn record_borrow_source(source: BorrowSource, highest_local_index: &mut Option<usize>) {
    match source {
        BorrowSource::I32(location) => record_i32_location(location, highest_local_index),
        BorrowSource::U8(location) => record_u8_location(location, highest_local_index),
        BorrowSource::Usize(location) => record_usize_location(location, highest_local_index),
        BorrowSource::Bool(location) => record_bool_location(location, highest_local_index),
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
        BoolComparisonOperator, CallTarget, FallibleFailureMode, ScalarArgument, SliceLocation,
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
