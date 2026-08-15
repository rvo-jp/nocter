use super::*;

pub(super) fn max_call_argument_count(instructions: &[Instruction]) -> usize {
    instructions
        .iter()
        .map(instruction_max_call_argument_count)
        .max()
        .unwrap_or(0)
}

pub(super) fn instruction_max_call_argument_count(instruction: &Instruction) -> usize {
    match instruction {
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
        | Instruction::CallStoredOutcome { arguments, .. }
        | Instruction::TailCall { arguments, .. } => {
            arguments.iter().map(ScalarArgument::abi_word_count).sum()
        }
        Instruction::DarwinSyscall { arguments, .. } => arguments.len() + 1,
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
        } => arguments
            .iter()
            .map(ScalarArgument::abi_word_count)
            .sum::<usize>()
            .max(failure_mode_max_call_argument_count(failure_mode)),
        Instruction::CallComposedOutcome {
            arguments,
            outer_mode,
            inner_mode,
            ..
        } => arguments
            .iter()
            .map(ScalarArgument::abi_word_count)
            .sum::<usize>()
            .max(failure_mode_max_call_argument_count(outer_mode))
            .max(failure_mode_max_call_argument_count(inner_mode)),
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
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            outcome_instructions,
            ..
        } => max_call_argument_count(success_instructions)
            .max(max_call_argument_count(outcome_instructions)),
        Instruction::CheckStoredFallible {
            success_instructions,
            failure_mode,
            ..
        } => max_call_argument_count(success_instructions)
            .max(failure_mode_max_call_argument_count(failure_mode)),
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
        | Instruction::ReturnOutcomeSuccess
        | Instruction::ReturnOptionalNone
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::Break
        | Instruction::Continue => 0,
        Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::CloseFd { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::CopyPointerBytes { .. }
        | Instruction::CopyAggregateToPointer { .. }
        | Instruction::CopyPointerToAggregate { .. }
        | Instruction::LoadU8FromPointer { .. }
        | Instruction::LoadI32FromPointer { .. }
        | Instruction::LoadUsizeFromPointer { .. }
        | Instruction::LoadIntegerFromPointer { .. }
        | Instruction::LoadBoolFromPointer { .. }
        | Instruction::LoadStrFromPointer { .. }
        | Instruction::CopySliceElementToAggregate { .. }
        | Instruction::CopyAggregateToSliceElement { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::StoreI32ToPointer { .. }
        | Instruction::StoreUsizeToPointer { .. }
        | Instruction::StoreIntegerToPointer { .. }
        | Instruction::StoreBoolToPointer { .. }
        | Instruction::StoreStrToPointer { .. }
        | Instruction::StoreU8ToSliceIndex { .. }
        | Instruction::StoreI32ToSliceIndex { .. }
        | Instruction::StoreUsizeToSliceIndex { .. }
        | Instruction::StoreIntegerToSliceIndex { .. }
        | Instruction::StoreBoolToSliceIndex { .. }
        | Instruction::StoreStrToSliceIndex { .. }
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::StoreAggregateUsize { .. }
        | Instruction::StoreAggregateInteger { .. }
        | Instruction::StoreAggregateIntegerIndexed { .. }
        | Instruction::StoreAggregateI32 { .. }
        | Instruction::StoreAggregateU16 { .. }
        | Instruction::StoreAggregateU32 { .. }
        | Instruction::StoreAggregateU8 { .. }
        | Instruction::StoreAggregateBool { .. }
        | Instruction::StoreAggregateUsizeIndexed { .. }
        | Instruction::StoreAggregateI32Indexed { .. }
        | Instruction::StoreAggregateU8Indexed { .. }
        | Instruction::StoreAggregateBoolIndexed { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateInteger { .. }
        | Instruction::LoadAggregateIntegerIndexed { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. }
        | Instruction::LoadAggregateUsizeIndexed { .. }
        | Instruction::LoadAggregateI32Indexed { .. }
        | Instruction::LoadAggregateU8Indexed { .. }
        | Instruction::LoadAggregateBoolIndexed { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::CopyAggregateProjected { .. }
        | Instruction::LoadStoredOutcomePayload { .. }
        | Instruction::ReturnStoredOutcome { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::RegionEnter { .. }
        | Instruction::SetCurrentAllocationContext { .. }
        | Instruction::RegionRelease { .. }
        | Instruction::SetUsizeFromBorrow { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetStrSubview { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSlice { .. }
        | Instruction::SetSliceRawParts { .. }
        | Instruction::AddU8 { .. }
        | Instruction::SubtractU8 { .. }
        | Instruction::MultiplyU8 { .. }
        | Instruction::DivideU8 { .. }
        | Instruction::RemainderU8 { .. }
        | Instruction::ShiftLeftU8 { .. }
        | Instruction::ShiftRightU8 { .. }
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
        | Instruction::IntegerBinary { .. }
        | Instruction::Trap
        | Instruction::Return => 0,
    }
}

pub(super) fn failure_mode_max_call_argument_count(failure_mode: &OutcomeFailureMode) -> usize {
    match failure_mode {
        OutcomeFailureMode::Propagate | OutcomeFailureMode::Trap => 0,
        OutcomeFailureMode::PropagateWithCleanup { instructions, .. }
        | OutcomeFailureMode::Handle { instructions }
        | OutcomeFailureMode::Recover { instructions }
        | OutcomeFailureMode::Catch { instructions, .. } => max_call_argument_count(instructions),
    }
}
