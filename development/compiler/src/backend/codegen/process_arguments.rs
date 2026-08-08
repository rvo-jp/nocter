use super::*;

pub(super) fn module_uses_process_arguments(module: &IrModule) -> bool {
    module
        .functions
        .iter()
        .any(|function| instructions_use_process_arguments(&function.instructions))
}

pub(super) fn instructions_use_process_arguments(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_uses_process_arguments)
}

pub(super) fn instruction_uses_process_arguments(instruction: &Instruction) -> bool {
    match instruction {
        Instruction::WriteStr { fd, text } => {
            i32_value_uses_process_arguments(fd) || str_value_uses_process_arguments(text)
        }
        Instruction::WriteSlice { fd, bytes } => {
            i32_value_uses_process_arguments(fd) || slice_value_uses_process_arguments(bytes)
        }
        Instruction::ReadSlice {
            fd,
            buffer,
            failure_mode,
            ..
        } => {
            i32_value_uses_process_arguments(fd)
                || slice_value_uses_process_arguments(buffer)
                || failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::OpenRead {
            path,
            flags,
            mode,
            failure_mode,
            ..
        } => {
            usize_value_uses_process_arguments(path)
                || usize_value_uses_process_arguments(flags)
                || usize_value_uses_process_arguments(mode)
                || failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::CloseFd { fd } | Instruction::ProcessExit { code: fd } => {
            i32_value_uses_process_arguments(fd)
        }
        Instruction::SetI32 { value, .. } => i32_value_uses_process_arguments(value),
        Instruction::SetU8 { value, .. } => u8_value_uses_process_arguments(value),
        Instruction::SetUsize { value, .. } => usize_value_uses_process_arguments(value),
        Instruction::RegionEnter { .. } => false,
        Instruction::SetCurrentAllocationContext { state, kind } => {
            usize_value_uses_process_arguments(state) || usize_value_uses_process_arguments(kind)
        }
        Instruction::RegionRelease {
            state,
            parent_state,
            parent_kind,
        } => {
            usize_value_uses_process_arguments(state)
                || usize_value_uses_process_arguments(parent_state)
                || usize_value_uses_process_arguments(parent_kind)
        }
        Instruction::SetUsizeFromBorrow { .. } => false,
        Instruction::SetBool { value, .. } => bool_value_uses_process_arguments(value),
        Instruction::SetStr { value, .. } => str_value_uses_process_arguments(value),
        Instruction::SetStrSubview {
            source, start, len, ..
        } => {
            str_value_uses_process_arguments(source)
                || usize_value_uses_process_arguments(start)
                || usize_value_uses_process_arguments(len)
        }
        Instruction::SetStrRawParts { pointer, len, .. }
        | Instruction::SetSliceRawParts { pointer, len, .. } => {
            usize_value_uses_process_arguments(pointer) || usize_value_uses_process_arguments(len)
        }
        Instruction::SetSlice { value, .. } => slice_value_uses_process_arguments(value),
        Instruction::ReserveAggregateSlot { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::CopySliceElementToAggregate { .. }
        | Instruction::CopyAggregateToSliceElement { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateInteger { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. } => false,
        Instruction::LoadAggregateUsizeIndexed { index, .. }
        | Instruction::LoadAggregateIntegerIndexed { index, .. }
        | Instruction::LoadAggregateI32Indexed { index, .. }
        | Instruction::LoadAggregateU8Indexed { index, .. }
        | Instruction::LoadAggregateBoolIndexed { index, .. } => {
            usize_value_uses_process_arguments(index)
        }
        Instruction::StoreAggregateUsize { value, .. } => usize_value_uses_process_arguments(value),
        Instruction::StoreAggregateInteger { value, .. } => {
            usize_value_uses_process_arguments(value)
        }
        Instruction::StoreAggregateIntegerIndexed { index, value, .. } => {
            usize_value_uses_process_arguments(index) || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreAggregateUsizeIndexed { index, value, .. } => {
            usize_value_uses_process_arguments(index) || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreAggregateI32 { value, .. } => i32_value_uses_process_arguments(value),
        Instruction::StoreAggregateI32Indexed { index, value, .. } => {
            usize_value_uses_process_arguments(index) || i32_value_uses_process_arguments(value)
        }
        Instruction::StoreAggregateU16 { .. } | Instruction::StoreAggregateU32 { .. } => false,
        Instruction::StoreAggregateU8 { value, .. } => u8_value_uses_process_arguments(value),
        Instruction::StoreAggregateU8Indexed { index, value, .. } => {
            usize_value_uses_process_arguments(index) || u8_value_uses_process_arguments(value)
        }
        Instruction::StoreAggregateBool { value, .. } => bool_value_uses_process_arguments(value),
        Instruction::StoreAggregateBoolIndexed { index, value, .. } => {
            usize_value_uses_process_arguments(index) || bool_value_uses_process_arguments(value)
        }
        Instruction::DarwinSyscall {
            number, arguments, ..
        } => {
            usize_value_uses_process_arguments(number)
                || arguments.iter().any(usize_value_uses_process_arguments)
        }
        Instruction::CopyStrToPointer {
            pointer,
            offset,
            text,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || str_value_uses_process_arguments(text)
        }
        Instruction::CopyPointerBytes {
            destination,
            source,
            byte_count,
        } => {
            usize_value_uses_process_arguments(destination)
                || usize_value_uses_process_arguments(source)
                || usize_value_uses_process_arguments(byte_count)
        }
        Instruction::CopyAggregateToPointer {
            pointer, offset, ..
        }
        | Instruction::CopyPointerToAggregate {
            pointer, offset, ..
        }
        | Instruction::LoadU8FromPointer {
            pointer, offset, ..
        }
        | Instruction::LoadI32FromPointer {
            pointer, offset, ..
        }
        | Instruction::LoadUsizeFromPointer {
            pointer, offset, ..
        }
        | Instruction::LoadIntegerFromPointer {
            pointer, offset, ..
        }
        | Instruction::LoadBoolFromPointer {
            pointer, offset, ..
        }
        | Instruction::LoadStrFromPointer {
            pointer, offset, ..
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
        }
        Instruction::StoreU8ToPointer {
            pointer,
            offset,
            value,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || u8_value_uses_process_arguments(value)
        }
        Instruction::StoreI32ToPointer {
            pointer,
            offset,
            value,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || i32_value_uses_process_arguments(value)
        }
        Instruction::StoreUsizeToPointer {
            pointer,
            offset,
            value,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreIntegerToPointer {
            pointer,
            offset,
            value,
            ..
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreBoolToPointer {
            pointer,
            offset,
            value,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || bool_value_uses_process_arguments(value)
        }
        Instruction::StoreStrToPointer {
            pointer,
            offset,
            value,
        } => {
            usize_value_uses_process_arguments(pointer)
                || usize_value_uses_process_arguments(offset)
                || str_value_uses_process_arguments(value)
        }
        Instruction::StoreU8ToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || u8_value_uses_process_arguments(value)
        }
        Instruction::StoreI32ToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || i32_value_uses_process_arguments(value)
        }
        Instruction::StoreUsizeToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreIntegerToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || usize_value_uses_process_arguments(value)
        }
        Instruction::StoreBoolToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || bool_value_uses_process_arguments(value)
        }
        Instruction::StoreStrToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || str_value_uses_process_arguments(value)
        }
        Instruction::AddU8 { left, right, .. }
        | Instruction::SubtractU8 { left, right, .. }
        | Instruction::MultiplyU8 { left, right, .. }
        | Instruction::DivideU8 { left, right, .. }
        | Instruction::RemainderU8 { left, right, .. }
        | Instruction::ShiftLeftU8 { left, right, .. }
        | Instruction::ShiftRightU8 { left, right, .. } => {
            u8_value_uses_process_arguments(left) || u8_value_uses_process_arguments(right)
        }
        Instruction::AddI32 { left, right, .. }
        | Instruction::SubtractI32 { left, right, .. }
        | Instruction::MultiplyI32 { left, right, .. }
        | Instruction::DivideI32 { left, right, .. }
        | Instruction::RemainderI32 { left, right, .. }
        | Instruction::ShiftLeftI32 { left, right, .. }
        | Instruction::ShiftRightI32 { left, right, .. } => {
            i32_value_uses_process_arguments(left) || i32_value_uses_process_arguments(right)
        }
        Instruction::AddUsize { left, right, .. }
        | Instruction::SubtractUsize { left, right, .. }
        | Instruction::MultiplyUsize { left, right, .. }
        | Instruction::DivideUsize { left, right, .. }
        | Instruction::RemainderUsize { left, right, .. }
        | Instruction::ShiftLeftUsize { left, right, .. }
        | Instruction::ShiftRightUsize { left, right, .. } => {
            usize_value_uses_process_arguments(left) || usize_value_uses_process_arguments(right)
        }
        Instruction::IntegerBinary { left, right, .. } => {
            usize_value_uses_process_arguments(left) || usize_value_uses_process_arguments(right)
        }
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallBorrow { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::TailCall { arguments, .. } => {
            scalar_arguments_use_process_arguments(arguments)
        }
        Instruction::CallOutcomeI32 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeU8 {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeUsize {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeBorrow {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeBool {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeStr {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeSlice {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeDirectAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeAggregate {
            arguments,
            failure_mode,
            ..
        }
        | Instruction::CallOutcomeVoid {
            arguments,
            failure_mode,
            ..
        } => {
            scalar_arguments_use_process_arguments(arguments)
                || failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::CallComposedOutcome {
            arguments,
            outer_mode,
            inner_mode,
            ..
        } => {
            scalar_arguments_use_process_arguments(arguments)
                || failure_mode_uses_process_arguments(outer_mode)
                || failure_mode_uses_process_arguments(inner_mode)
        }
        Instruction::CallStoredOutcome { arguments, .. } => {
            scalar_arguments_use_process_arguments(arguments)
        }
        Instruction::If {
            condition,
            then_instructions,
            else_instructions,
        } => {
            bool_value_uses_process_arguments(condition)
                || instructions_use_process_arguments(then_instructions)
                || instructions_use_process_arguments(else_instructions)
        }
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            outcome_instructions,
            ..
        } => {
            instructions_use_process_arguments(success_instructions)
                || instructions_use_process_arguments(outcome_instructions)
        }
        Instruction::CheckStoredFallible {
            success_instructions,
            failure_mode,
            ..
        } => {
            instructions_use_process_arguments(success_instructions)
                || failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::While {
            condition_instructions,
            condition,
            body_instructions,
        } => {
            instructions_use_process_arguments(condition_instructions)
                || bool_value_uses_process_arguments(condition)
                || instructions_use_process_arguments(body_instructions)
        }
        Instruction::CheckFailure { failure_mode } => {
            failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::ReturnFallibleFailure { code, message } => {
            str_value_uses_process_arguments(code) || str_value_uses_process_arguments(message)
        }
        Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnOutcomeSuccess
        | Instruction::ReturnOptionalNone
        | Instruction::LoadStoredOutcomePayload { .. }
        | Instruction::ReturnStoredOutcome { .. }
        | Instruction::Return => false,
    }
}

pub(super) fn failure_mode_uses_process_arguments(failure_mode: &OutcomeFailureMode) -> bool {
    match failure_mode {
        OutcomeFailureMode::Propagate | OutcomeFailureMode::Trap => false,
        OutcomeFailureMode::PropagateWithCleanup { instructions, .. }
        | OutcomeFailureMode::Handle { instructions }
        | OutcomeFailureMode::Recover { instructions }
        | OutcomeFailureMode::Catch { instructions, .. } => {
            instructions_use_process_arguments(instructions)
        }
    }
}

pub(super) fn scalar_arguments_use_process_arguments(arguments: &[ScalarArgument]) -> bool {
    arguments.iter().any(scalar_argument_uses_process_arguments)
}

pub(super) fn scalar_argument_uses_process_arguments(argument: &ScalarArgument) -> bool {
    match argument {
        ScalarArgument::I32(value) => i32_value_uses_process_arguments(value),
        ScalarArgument::U8(value) => u8_value_uses_process_arguments(value),
        ScalarArgument::Usize(value) => usize_value_uses_process_arguments(value),
        ScalarArgument::Bool(value) => bool_value_uses_process_arguments(value),
        ScalarArgument::Str(value) => str_value_uses_process_arguments(value),
        ScalarArgument::Slice(value) => slice_value_uses_process_arguments(value),
        ScalarArgument::Borrow(_)
        | ScalarArgument::AggregateIndirect(_)
        | ScalarArgument::AggregateDirect(_) => false,
    }
}

pub(super) fn i32_value_uses_process_arguments(value: &I32Value) -> bool {
    match value {
        I32Value::Const(_) | I32Value::Location(_) => false,
        I32Value::U8ZeroExtend(value) => u8_value_uses_process_arguments(value),
        I32Value::IntegerWord(value) => usize_value_uses_process_arguments(value),
        I32Value::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

pub(super) fn u8_value_uses_process_arguments(value: &crate::ir::U8Value) -> bool {
    match value {
        crate::ir::U8Value::Const(_) | crate::ir::U8Value::Location(_) => false,
        crate::ir::U8Value::StrIndex { index, .. }
        | crate::ir::U8Value::StaticStrIndex { index, .. }
        | crate::ir::U8Value::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

pub(super) fn usize_value_uses_process_arguments(value: &UsizeValue) -> bool {
    match value {
        UsizeValue::ProcessArgCount | UsizeValue::ProcessEnvironmentCount => true,
        UsizeValue::Const(_)
        | UsizeValue::Location(_)
        | UsizeValue::CurrentAllocationState
        | UsizeValue::CurrentAllocationKind => false,
        UsizeValue::U8ZeroExtend(value) => u8_value_uses_process_arguments(value),
        UsizeValue::I32SignExtend(value) => i32_value_uses_process_arguments(value),
        UsizeValue::StrPointer(_)
        | UsizeValue::SlicePointer(_)
        | UsizeValue::StrLen(_)
        | UsizeValue::SliceLen(_) => false,
        UsizeValue::SliceIndex { index, .. } | UsizeValue::IntegerSliceIndex { index, .. } => {
            usize_value_uses_process_arguments(index)
        }
    }
}

pub(super) fn bool_value_uses_process_arguments(value: &crate::ir::BoolValue) -> bool {
    match value {
        crate::ir::BoolValue::Const(_) | crate::ir::BoolValue::Location(_) => false,
        crate::ir::BoolValue::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
        crate::ir::BoolValue::Not(value) => bool_value_uses_process_arguments(value),
        crate::ir::BoolValue::Logical { left, right, .. }
        | crate::ir::BoolValue::BoolComparison { left, right, .. } => {
            bool_value_uses_process_arguments(left) || bool_value_uses_process_arguments(right)
        }
        crate::ir::BoolValue::I32Comparison { left, right, .. } => {
            i32_value_uses_process_arguments(left) || i32_value_uses_process_arguments(right)
        }
        crate::ir::BoolValue::UsizeComparison { left, right, .. } => {
            usize_value_uses_process_arguments(left) || usize_value_uses_process_arguments(right)
        }
        crate::ir::BoolValue::IntegerComparison { left, right, .. } => {
            usize_value_uses_process_arguments(left) || usize_value_uses_process_arguments(right)
        }
        crate::ir::BoolValue::StrComparison { left, right, .. } => {
            str_value_uses_process_arguments(left) || str_value_uses_process_arguments(right)
        }
    }
}

pub(super) fn str_value_uses_process_arguments(value: &StrValue) -> bool {
    match value {
        StrValue::ProcessArg { .. }
        | StrValue::ProcessEnvironmentName { .. }
        | StrValue::ProcessEnvironmentValue { .. } => true,
        StrValue::StaticBytes(_) | StrValue::Location(_) => false,
        StrValue::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

pub(super) fn slice_value_uses_process_arguments(value: &SliceValue) -> bool {
    match value {
        SliceValue::Location(_) => false,
        SliceValue::StrBytes(text) => str_value_uses_process_arguments(text),
    }
}
