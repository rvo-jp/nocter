use super::*;

pub(super) fn parameter_spill_requests(
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

pub(super) fn record_i32_value_parameter_spill_requests(
    value: &I32Value,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(I32Location::Parameter(index)) => {
            requests.insert(*index);
        }
        I32Value::Location(I32Location::Return | I32Location::Local(_)) => {}
        I32Value::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        I32Value::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

pub(super) fn record_u8_value_parameter_spill_requests(
    value: &U8Value,
    requests: &mut BTreeSet<usize>,
) {
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

pub(super) fn record_usize_value_parameter_spill_requests(
    value: &UsizeValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        UsizeValue::Const(_) | UsizeValue::ProcessArgCount => {}
        UsizeValue::Location(UsizeLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        UsizeValue::Location(UsizeLocation::Return | UsizeLocation::Local(_)) => {}
        UsizeValue::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        UsizeValue::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
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

pub(super) fn record_bool_value_parameter_spill_requests(
    value: &BoolValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(BoolLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        BoolValue::Location(BoolLocation::Return | BoolLocation::Local(_)) => {}
        BoolValue::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
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
        BoolValue::StrComparison { left, right, .. } => {
            record_str_value_parameter_spill_requests(left, requests);
            record_str_value_parameter_spill_requests(right, requests);
        }
    }
}

pub(super) fn record_str_value_parameter_spill_requests(
    value: &StrValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        StrValue::StaticBytes(_) => {}
        StrValue::Location(location) => {
            record_str_location_parameter_pair_spill_requests(*location, requests);
        }
        StrValue::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        StrValue::ProcessArg { index } => {
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

pub(super) fn record_slice_value_parameter_spill_requests(
    value: &SliceValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        SliceValue::StrBytes(text) => {
            record_str_value_parameter_spill_requests(text, requests);
        }
        SliceValue::Location(location) => {
            record_slice_location_parameter_pair_spill_requests(*location, requests);
        }
    }
}

pub(super) fn record_str_location_parameter_pair_spill_requests(
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

pub(super) fn record_slice_location_parameter_pair_spill_requests(
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

pub(super) fn record_aggregate_location_parameter_spill_request(
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

pub(super) fn record_borrow_source_parameter_spill_request(
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
        BorrowSource::BorrowParameter(index)
        | BorrowSource::AggregateParameter(index)
        | BorrowSource::AggregateParameterField {
            parameter_index: index,
            ..
        } => {
            requests.insert(index);
        }
        BorrowSource::SliceIndex { source, index, .. } => {
            record_slice_location_parameter_pair_spill_requests(source, requests);
            record_slice_element_index_parameter_spill_request(index, requests);
        }
        BorrowSource::I32(I32Location::Return | I32Location::Local(_))
        | BorrowSource::U8(U8Location::Return | U8Location::Local(_))
        | BorrowSource::Usize(UsizeLocation::Return | UsizeLocation::Local(_))
        | BorrowSource::Bool(BoolLocation::Return | BoolLocation::Local(_))
        | BorrowSource::AggregateSlot(_)
        | BorrowSource::AggregateSlotField { .. } => {}
    }
}

pub(super) fn record_slice_element_index_parameter_spill_request(
    index: SliceElementIndex,
    requests: &mut BTreeSet<usize>,
) {
    if let SliceElementIndex::Location(UsizeLocation::Parameter(index)) = index {
        requests.insert(index);
    }
}
