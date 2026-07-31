use super::*;

pub(in crate::backend::frame) fn parameter_spill_requests(
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

pub(super) fn record_instruction_list_parameter_spill_requests(
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

pub(super) fn record_instruction_parameter_spill_requests(
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
        Instruction::CopyPointerBytes {
            destination,
            source,
            byte_count,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(destination, requests);
                record_usize_value_parameter_spill_requests(source, requests);
                record_usize_value_parameter_spill_requests(byte_count, requests);
            }
        }
        Instruction::CopyAggregateToPointer {
            pointer, offset, ..
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
            }
        }
        Instruction::CopySliceElementToAggregate {
            destination,
            source,
            index,
            layout,
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    0,
                    layout.size,
                    requests,
                );
                record_slice_location_parameter_pair_spill_requests(*source, requests);
                record_slice_element_index_parameter_spill_request(*index, requests);
            }
        }
        Instruction::CopyAggregateToSliceElement {
            destination,
            index,
            source,
            layout,
        } => {
            if include_value_parameters {
                record_slice_location_parameter_pair_spill_requests(*destination, requests);
                record_slice_element_index_parameter_spill_request(*index, requests);
                record_aggregate_location_parameter_spill_request(
                    *source,
                    0,
                    layout.size,
                    requests,
                );
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
        Instruction::StoreI32ToPointer {
            pointer,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_i32_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreUsizeToPointer {
            pointer,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_usize_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreBoolToPointer {
            pointer,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_bool_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreStrToPointer {
            pointer,
            offset,
            value,
        } => {
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(pointer, requests);
                record_usize_value_parameter_spill_requests(offset, requests);
                record_str_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreU8ToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location_parameter_pair_spill_requests(*destination, requests);
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(index, requests);
                record_u8_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreI32ToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location_parameter_pair_spill_requests(*destination, requests);
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(index, requests);
                record_i32_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreUsizeToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location_parameter_pair_spill_requests(*destination, requests);
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(index, requests);
                record_usize_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreBoolToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location_parameter_pair_spill_requests(*destination, requests);
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(index, requests);
                record_bool_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreStrToSliceIndex {
            destination,
            index,
            value,
        } => {
            record_slice_location_parameter_pair_spill_requests(*destination, requests);
            if include_value_parameters {
                record_usize_value_parameter_spill_requests(index, requests);
                record_str_value_parameter_spill_requests(value, requests);
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
        Instruction::StoreAggregateUsizeIndexed {
            destination,
            base_offset,
            index,
            value,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *base_offset,
                    8,
                    requests,
                );
                record_usize_value_parameter_spill_requests(index, requests);
                record_usize_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateI32Indexed {
            destination,
            base_offset,
            index,
            value,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *base_offset,
                    4,
                    requests,
                );
                record_usize_value_parameter_spill_requests(index, requests);
                record_i32_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateU8Indexed {
            destination,
            base_offset,
            index,
            value,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *base_offset,
                    1,
                    requests,
                );
                record_usize_value_parameter_spill_requests(index, requests);
                record_u8_value_parameter_spill_requests(value, requests);
            }
        }
        Instruction::StoreAggregateBoolIndexed {
            destination,
            base_offset,
            index,
            value,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *destination,
                    *base_offset,
                    1,
                    requests,
                );
                record_usize_value_parameter_spill_requests(index, requests);
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
        Instruction::LoadAggregateUsizeIndexed {
            source,
            base_offset,
            index,
            ..
        }
        | Instruction::LoadAggregateI32Indexed {
            source,
            base_offset,
            index,
            ..
        }
        | Instruction::LoadAggregateU8Indexed {
            source,
            base_offset,
            index,
            ..
        }
        | Instruction::LoadAggregateBoolIndexed {
            source,
            base_offset,
            index,
            ..
        } => {
            if include_value_parameters {
                record_aggregate_location_parameter_spill_request(
                    *source,
                    *base_offset,
                    1,
                    requests,
                );
                record_usize_value_parameter_spill_requests(index, requests);
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
        Instruction::SetUsizeFromBorrow { source, .. } => {
            record_borrow_source_parameter_spill_request(*source, requests);
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
        Instruction::AddU8 { left, right, .. }
        | Instruction::SubtractU8 { left, right, .. }
        | Instruction::MultiplyU8 { left, right, .. }
        | Instruction::DivideU8 { left, right, .. }
        | Instruction::RemainderU8 { left, right, .. }
        | Instruction::ShiftLeftU8 { left, right, .. }
        | Instruction::ShiftRightU8 { left, right, .. } => {
            if include_value_parameters {
                record_u8_value_parameter_spill_requests(left, requests);
                record_u8_value_parameter_spill_requests(right, requests);
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
        | Instruction::ReturnOptionalNone
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => {}
    }
}

pub(super) fn record_failure_mode_parameter_spill_requests(
    failure_mode: &FallibleFailureMode,
    requests: &mut BTreeSet<usize>,
    include_value_parameters: bool,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            record_instruction_list_parameter_spill_requests(
                instructions,
                requests,
                include_value_parameters,
            );
        }
    }
}

pub(super) fn record_scalar_arguments_parameter_spill_requests(
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

pub(super) fn record_scalar_argument_parameter_spill_requests(
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
