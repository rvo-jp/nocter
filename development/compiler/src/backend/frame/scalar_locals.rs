use super::*;

pub(super) fn scalar_spill_slot_count(instructions: &[Instruction]) -> usize {
    let mut highest_local_index = None;
    record_instruction_list_scalar_locals(instructions, &mut highest_local_index);
    highest_local_index.map_or(0, |index| index + 1)
}

pub(super) fn record_instruction_list_scalar_locals(
    instructions: &[Instruction],
    highest_local_index: &mut Option<usize>,
) {
    for instruction in instructions {
        record_instruction_scalar_locals(instruction, highest_local_index);
    }
}

pub(super) fn record_instruction_scalar_locals(
    instruction: &Instruction,
    highest_local_index: &mut Option<usize>,
) {
    match instruction {
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnOutcomeSuccess
        | Instruction::ReturnOptionalNone
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
        Instruction::StoreAggregateUsizeIndexed { index, value, .. } => {
            record_usize_value(index, highest_local_index);
            record_usize_value(value, highest_local_index);
        }
        Instruction::StoreAggregateI32 { value, .. } => {
            record_i32_value(value, highest_local_index);
        }
        Instruction::StoreAggregateI32Indexed { index, value, .. } => {
            record_usize_value(index, highest_local_index);
            record_i32_value(value, highest_local_index);
        }
        Instruction::StoreAggregateU16 { .. } => {}
        Instruction::StoreAggregateU32 { .. } => {}
        Instruction::StoreAggregateU8 { value, .. } => {
            record_u8_value(value, highest_local_index);
        }
        Instruction::StoreAggregateU8Indexed { index, value, .. } => {
            record_usize_value(index, highest_local_index);
            record_u8_value(value, highest_local_index);
        }
        Instruction::StoreAggregateBool { value, .. } => {
            record_bool_value(value, highest_local_index);
        }
        Instruction::StoreAggregateBoolIndexed { index, value, .. } => {
            record_usize_value(index, highest_local_index);
            record_bool_value(value, highest_local_index);
        }
        Instruction::LoadAggregateUsize { destination, .. } => {
            record_usize_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateUsizeIndexed {
            destination, index, ..
        } => {
            record_usize_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
        Instruction::LoadAggregateI32 { destination, .. } => {
            record_i32_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateI32Indexed {
            destination, index, ..
        } => {
            record_i32_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
        Instruction::LoadAggregateU8 { destination, .. } => {
            record_u8_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateU8Indexed {
            destination, index, ..
        } => {
            record_u8_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
        Instruction::LoadAggregateBool { destination, .. } => {
            record_bool_location(*destination, highest_local_index);
        }
        Instruction::LoadAggregateBoolIndexed {
            destination, index, ..
        } => {
            record_bool_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
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
            flags,
            mode,
            failure_mode,
        } => {
            record_i32_location(*destination, highest_local_index);
            record_usize_value(path, highest_local_index);
            record_usize_value(flags, highest_local_index);
            record_usize_value(mode, highest_local_index);
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
        Instruction::CopyPointerBytes {
            destination,
            source,
            byte_count,
        } => {
            record_usize_value(destination, highest_local_index);
            record_usize_value(source, highest_local_index);
            record_usize_value(byte_count, highest_local_index);
        }
        Instruction::CopyAggregateToPointer {
            pointer, offset, ..
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::CopyPointerToAggregate {
            pointer, offset, ..
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::LoadU8FromPointer {
            destination,
            pointer,
            offset,
        } => {
            record_u8_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::LoadI32FromPointer {
            destination,
            pointer,
            offset,
        } => {
            record_i32_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::LoadUsizeFromPointer {
            destination,
            pointer,
            offset,
        } => {
            record_usize_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::LoadBoolFromPointer {
            destination,
            pointer,
            offset,
        } => {
            record_bool_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::LoadStrFromPointer {
            destination,
            pointer,
            offset,
        } => {
            record_str_location(*destination, highest_local_index);
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
        }
        Instruction::CopySliceElementToAggregate { source, index, .. } => {
            record_slice_location(*source, highest_local_index);
            record_slice_element_index(*index, highest_local_index);
        }
        Instruction::CopyAggregateToSliceElement {
            destination, index, ..
        } => {
            record_slice_location(*destination, highest_local_index);
            record_slice_element_index(*index, highest_local_index);
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
        Instruction::StoreI32ToPointer {
            pointer,
            offset,
            value,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_i32_value(value, highest_local_index);
        }
        Instruction::StoreUsizeToPointer {
            pointer,
            offset,
            value,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_usize_value(value, highest_local_index);
        }
        Instruction::StoreBoolToPointer {
            pointer,
            offset,
            value,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_bool_value(value, highest_local_index);
        }
        Instruction::StoreStrToPointer {
            pointer,
            offset,
            value,
        } => {
            record_usize_value(pointer, highest_local_index);
            record_usize_value(offset, highest_local_index);
            record_str_value(value, highest_local_index);
        }
        Instruction::StoreU8ToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
            record_u8_value(value, highest_local_index);
        }
        Instruction::StoreI32ToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
            record_i32_value(value, highest_local_index);
        }
        Instruction::StoreUsizeToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
            record_usize_value(value, highest_local_index);
        }
        Instruction::StoreBoolToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
            record_bool_value(value, highest_local_index);
        }
        Instruction::StoreStrToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location(*destination, highest_local_index);
            record_usize_value(index, highest_local_index);
            record_str_value(value, highest_local_index);
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
        Instruction::RegionEnter { destination } => {
            record_usize_location(*destination, highest_local_index);
        }
        Instruction::SetCurrentAllocationContext { state, kind } => {
            record_usize_value(state, highest_local_index);
            record_usize_value(kind, highest_local_index);
        }
        Instruction::RegionRelease {
            state,
            parent_state,
            parent_kind,
        } => {
            record_usize_value(state, highest_local_index);
            record_usize_value(parent_state, highest_local_index);
            record_usize_value(parent_kind, highest_local_index);
        }
        Instruction::SetUsizeFromBorrow {
            destination,
            source,
        } => {
            record_usize_location(*destination, highest_local_index);
            record_borrow_source(*source, highest_local_index);
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
        Instruction::AddU8 {
            destination,
            left,
            right,
        }
        | Instruction::SubtractU8 {
            destination,
            left,
            right,
        }
        | Instruction::MultiplyU8 {
            destination,
            left,
            right,
        }
        | Instruction::DivideU8 {
            destination,
            left,
            right,
        }
        | Instruction::RemainderU8 {
            destination,
            left,
            right,
        }
        | Instruction::ShiftLeftU8 {
            destination,
            left,
            right,
        }
        | Instruction::ShiftRightU8 {
            destination,
            left,
            right,
        } => {
            record_u8_location(*destination, highest_local_index);
            record_u8_value(left, highest_local_index);
            record_u8_value(right, highest_local_index);
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
        Instruction::CallOutcomeI32 {
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
        Instruction::CallOutcomeU8 {
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
        Instruction::CallBorrow {
            destination,
            arguments,
            ..
        } => {
            record_usize_location(*destination, highest_local_index);
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
        }
        Instruction::CallOutcomeUsize {
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
        Instruction::CallOutcomeBorrow {
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
        Instruction::CallOutcomeBool {
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
        Instruction::CallOutcomeStr {
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
        Instruction::CallOutcomeSlice {
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
        Instruction::CallOutcomeDirectAggregate {
            arguments,
            failure_mode,
            ..
        } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallOutcomeAggregate {
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
        Instruction::CallOutcomeVoid {
            arguments,
            failure_mode,
            ..
        } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::CallComposedOutcome {
            destination,
            arguments,
            outer_mode,
            inner_mode,
            ..
        } => {
            match destination {
                crate::ir::ComposedOutcomeDestination::I32(location) => {
                    record_i32_location(*location, highest_local_index)
                }
                crate::ir::ComposedOutcomeDestination::U8(location) => {
                    record_u8_location(*location, highest_local_index)
                }
                crate::ir::ComposedOutcomeDestination::Usize(location)
                | crate::ir::ComposedOutcomeDestination::Borrow(location) => {
                    record_usize_location(*location, highest_local_index)
                }
                crate::ir::ComposedOutcomeDestination::Bool(location) => {
                    record_bool_location(*location, highest_local_index)
                }
                crate::ir::ComposedOutcomeDestination::Str(location) => {
                    record_str_location(*location, highest_local_index)
                }
                crate::ir::ComposedOutcomeDestination::Slice(location) => {
                    record_slice_location(*location, highest_local_index)
                }
            }
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
            record_failure_mode_scalar_locals(outer_mode, highest_local_index);
            record_failure_mode_scalar_locals(inner_mode, highest_local_index);
        }
        Instruction::CallStoredOutcome { arguments, .. } => {
            for argument in arguments {
                record_scalar_argument(argument, highest_local_index);
            }
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
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            outcome_instructions,
            ..
        } => {
            record_instruction_list_scalar_locals(success_instructions, highest_local_index);
            record_instruction_list_scalar_locals(outcome_instructions, highest_local_index);
        }
        Instruction::CheckStoredFallible {
            success_instructions,
            failure_mode,
            ..
        } => {
            record_instruction_list_scalar_locals(success_instructions, highest_local_index);
            record_failure_mode_scalar_locals(failure_mode, highest_local_index);
        }
        Instruction::LoadStoredOutcomePayload { destination, .. } => match destination {
            crate::ir::ComposedOutcomeDestination::I32(location) => {
                record_i32_location(*location, highest_local_index)
            }
            crate::ir::ComposedOutcomeDestination::U8(location) => {
                record_u8_location(*location, highest_local_index)
            }
            crate::ir::ComposedOutcomeDestination::Usize(location)
            | crate::ir::ComposedOutcomeDestination::Borrow(location) => {
                record_usize_location(*location, highest_local_index)
            }
            crate::ir::ComposedOutcomeDestination::Bool(location) => {
                record_bool_location(*location, highest_local_index)
            }
            crate::ir::ComposedOutcomeDestination::Str(location) => {
                record_str_location(*location, highest_local_index)
            }
            crate::ir::ComposedOutcomeDestination::Slice(location) => {
                record_slice_location(*location, highest_local_index)
            }
        },
        Instruction::ReturnStoredOutcome { .. } => {}
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

pub(super) fn record_failure_mode_scalar_locals(
    failure_mode: &OutcomeFailureMode,
    highest_local_index: &mut Option<usize>,
) {
    match failure_mode {
        OutcomeFailureMode::Propagate | OutcomeFailureMode::Trap => {}
        OutcomeFailureMode::PropagateWithCleanup {
            code,
            message,
            instructions,
        } => {
            record_str_location(*code, highest_local_index);
            record_str_location(*message, highest_local_index);
            record_instruction_list_scalar_locals(instructions, highest_local_index);
        }
        OutcomeFailureMode::Handle { instructions } => {
            record_instruction_list_scalar_locals(instructions, highest_local_index);
        }
        OutcomeFailureMode::Recover { instructions } => {
            record_instruction_list_scalar_locals(instructions, highest_local_index);
        }
        OutcomeFailureMode::Catch {
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

pub(super) fn record_i32_value(value: &I32Value, highest_local_index: &mut Option<usize>) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(location) => record_i32_location(*location, highest_local_index),
        I32Value::U8ZeroExtend(value) => record_u8_value(value, highest_local_index),
        I32Value::SliceIndex { source, index } => {
            record_slice_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
    }
}

pub(super) fn record_scalar_argument(
    argument: &ScalarArgument,
    highest_local_index: &mut Option<usize>,
) {
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

pub(super) fn record_borrow_source(source: BorrowSource, highest_local_index: &mut Option<usize>) {
    match source {
        BorrowSource::I32(location) => record_i32_location(location, highest_local_index),
        BorrowSource::U8(location) => record_u8_location(location, highest_local_index),
        BorrowSource::Usize(location) => record_usize_location(location, highest_local_index),
        BorrowSource::Bool(location) => record_bool_location(location, highest_local_index),
        BorrowSource::BorrowParameter(_) => {}
        BorrowSource::BorrowLocal(location) => record_usize_location(location, highest_local_index),
        BorrowSource::SliceIndex { source, index, .. } => {
            record_slice_location(source, highest_local_index);
            record_slice_element_index(index, highest_local_index);
        }
        BorrowSource::PointerOffset {
            pointer, offset, ..
        } => {
            record_usize_location(pointer, highest_local_index);
            record_usize_location(offset, highest_local_index);
        }
        BorrowSource::AggregateSlot(_)
        | BorrowSource::AggregateSlotField { .. }
        | BorrowSource::AggregateParameter(_)
        | BorrowSource::AggregateParameterField { .. } => {}
    }
}

pub(super) fn record_slice_element_index(
    index: SliceElementIndex,
    highest_local_index: &mut Option<usize>,
) {
    if let SliceElementIndex::Location(location) = index {
        record_usize_location(location, highest_local_index);
    }
}

pub(super) fn record_i32_location(location: I32Location, highest_local_index: &mut Option<usize>) {
    if let I32Location::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

pub(super) fn record_u8_value(value: &U8Value, highest_local_index: &mut Option<usize>) {
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

pub(super) fn record_u8_location(location: U8Location, highest_local_index: &mut Option<usize>) {
    if let U8Location::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

pub(super) fn record_usize_value(value: &UsizeValue, highest_local_index: &mut Option<usize>) {
    match value {
        UsizeValue::Const(_)
        | UsizeValue::ProcessArgCount
        | UsizeValue::ProcessEnvironmentCount
        | UsizeValue::CurrentAllocationState
        | UsizeValue::CurrentAllocationKind => {}
        UsizeValue::Location(location) => record_usize_location(*location, highest_local_index),
        UsizeValue::U8ZeroExtend(value) => record_u8_value(value, highest_local_index),
        UsizeValue::StrLen(location) => record_str_location(*location, highest_local_index),
        UsizeValue::SliceLen(location) => record_slice_location(*location, highest_local_index),
        UsizeValue::SliceIndex { source, index } => {
            record_slice_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
    }
}

pub(super) fn record_usize_location(
    location: UsizeLocation,
    highest_local_index: &mut Option<usize>,
) {
    if let UsizeLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

pub(super) fn record_bool_value(value: &BoolValue, highest_local_index: &mut Option<usize>) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(location) => record_bool_location(*location, highest_local_index),
        BoolValue::SliceIndex { source, index } => {
            record_slice_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
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
        BoolValue::StrComparison { left, right, .. } => {
            record_str_value(left, highest_local_index);
            record_str_value(right, highest_local_index);
        }
        BoolValue::BoolComparison { left, right, .. } => {
            record_bool_value(left, highest_local_index);
            record_bool_value(right, highest_local_index);
        }
    }
}

pub(super) fn record_bool_location(
    location: BoolLocation,
    highest_local_index: &mut Option<usize>,
) {
    if let BoolLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

pub(super) fn record_str_value(value: &StrValue, highest_local_index: &mut Option<usize>) {
    match value {
        StrValue::StaticBytes(_) => {}
        StrValue::Location(location) => record_str_location(*location, highest_local_index),
        StrValue::SliceIndex { source, index } => {
            record_slice_location(*source, highest_local_index);
            record_usize_value(index, highest_local_index);
        }
        StrValue::ProcessArg { index }
        | StrValue::ProcessEnvironmentName { index }
        | StrValue::ProcessEnvironmentValue { index } => {
            record_usize_value(index, highest_local_index);
        }
    }
}

pub(super) fn record_str_location(location: StrLocation, highest_local_index: &mut Option<usize>) {
    if let StrLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
        record_scalar_local(index + 1, highest_local_index);
    }
}

pub(super) fn record_slice_value(value: &SliceValue, highest_local_index: &mut Option<usize>) {
    match value {
        SliceValue::StrBytes(text) => record_str_value(text, highest_local_index),
        SliceValue::Location(location) => record_slice_location(*location, highest_local_index),
    }
}

pub(super) fn record_slice_location(
    location: SliceLocation,
    highest_local_index: &mut Option<usize>,
) {
    if let SliceLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
        record_scalar_local(index + 1, highest_local_index);
    }
}

pub(super) fn record_scalar_local(index: usize, highest_local_index: &mut Option<usize>) {
    *highest_local_index = Some(highest_local_index.map_or(index, |highest| highest.max(index)));
}
