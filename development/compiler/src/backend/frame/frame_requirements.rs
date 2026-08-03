use super::*;

pub(super) fn function_requires_frame(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_requires_frame)
}

pub(super) fn function_clobbers_parameter_registers(instructions: &[Instruction]) -> bool {
    instructions
        .iter()
        .any(instruction_clobbers_parameter_registers)
}

pub(super) fn instruction_clobbers_parameter_registers(instruction: &Instruction) -> bool {
    match instruction {
        Instruction::CallI32 { .. }
        | Instruction::CallFallibleI32 { .. }
        | Instruction::CallU8 { .. }
        | Instruction::CallFallibleU8 { .. }
        | Instruction::CallUsize { .. }
        | Instruction::CallFallibleUsize { .. }
        | Instruction::CallBorrow { .. }
        | Instruction::CallFallibleBorrow { .. }
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
        | Instruction::CallComposedOutcome { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::ReadSlice { .. }
        | Instruction::OpenRead { .. }
        | Instruction::CloseFd { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::RegionEnter { .. }
        | Instruction::RegionRelease { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::CopyPointerBytes { .. }
        | Instruction::CopyAggregateToPointer { .. }
        | Instruction::CopySliceElementToAggregate { .. }
        | Instruction::CopyAggregateToSliceElement { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::StoreI32ToPointer { .. }
        | Instruction::StoreUsizeToPointer { .. }
        | Instruction::StoreBoolToPointer { .. }
        | Instruction::StoreStrToPointer { .. }
        | Instruction::StoreU8ToSliceIndex { .. }
        | Instruction::StoreI32ToSliceIndex { .. }
        | Instruction::StoreUsizeToSliceIndex { .. }
        | Instruction::StoreBoolToSliceIndex { .. }
        | Instruction::StoreStrToSliceIndex { .. } => true,
        Instruction::CopyPointerToAggregate { .. }
        | Instruction::LoadU8FromPointer { .. }
        | Instruction::LoadI32FromPointer { .. }
        | Instruction::LoadUsizeFromPointer { .. }
        | Instruction::LoadBoolFromPointer { .. }
        | Instruction::LoadStrFromPointer { .. } => false,
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
        | Instruction::ReturnOptionalNone
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
        | Instruction::StoreAggregateUsizeIndexed { .. }
        | Instruction::StoreAggregateI32Indexed { .. }
        | Instruction::StoreAggregateU8Indexed { .. }
        | Instruction::StoreAggregateBoolIndexed { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. }
        | Instruction::LoadAggregateUsizeIndexed { .. }
        | Instruction::LoadAggregateI32Indexed { .. }
        | Instruction::LoadAggregateU8Indexed { .. }
        | Instruction::LoadAggregateBoolIndexed { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetCurrentAllocationContext { .. }
        | Instruction::SetUsizeFromBorrow { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
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
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => false,
    }
}

pub(super) fn failure_mode_clobbers_parameter_registers(
    failure_mode: &FallibleFailureMode,
) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            function_clobbers_parameter_registers(instructions)
        }
    }
}

pub(super) fn instruction_requires_frame(instruction: &Instruction) -> bool {
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
        | Instruction::CallBorrow { .. }
        | Instruction::CallFallibleBorrow { .. }
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
        | Instruction::CallComposedOutcome { .. }
        | Instruction::ReserveAggregateSlot { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::ReadSlice { .. }
        | Instruction::OpenRead { .. }
        | Instruction::CloseFd { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::RegionEnter { .. }
        | Instruction::RegionRelease { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::CopyPointerBytes { .. }
        | Instruction::CopyAggregateToPointer { .. }
        | Instruction::CopyPointerToAggregate { .. }
        | Instruction::CopySliceElementToAggregate { .. }
        | Instruction::CopyAggregateToSliceElement { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::StoreI32ToPointer { .. }
        | Instruction::StoreUsizeToPointer { .. }
        | Instruction::StoreBoolToPointer { .. }
        | Instruction::StoreStrToPointer { .. }
        | Instruction::StoreU8ToSliceIndex { .. }
        | Instruction::StoreI32ToSliceIndex { .. }
        | Instruction::StoreUsizeToSliceIndex { .. }
        | Instruction::StoreBoolToSliceIndex { .. }
        | Instruction::StoreStrToSliceIndex { .. } => true,
        Instruction::LoadU8FromPointer { .. }
        | Instruction::LoadI32FromPointer { .. }
        | Instruction::LoadUsizeFromPointer { .. }
        | Instruction::LoadBoolFromPointer { .. }
        | Instruction::LoadStrFromPointer { .. } => false,
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
        | Instruction::StoreAggregateBool { destination, .. }
        | Instruction::StoreAggregateUsizeIndexed { destination, .. }
        | Instruction::StoreAggregateI32Indexed { destination, .. }
        | Instruction::StoreAggregateU8Indexed { destination, .. }
        | Instruction::StoreAggregateBoolIndexed { destination, .. } => {
            matches!(destination, AggregateLocation::Slot(_))
        }
        Instruction::LoadAggregateUsize { source, .. }
        | Instruction::LoadAggregateI32 { source, .. }
        | Instruction::LoadAggregateU8 { source, .. }
        | Instruction::LoadAggregateBool { source, .. }
        | Instruction::LoadAggregateUsizeIndexed { source, .. }
        | Instruction::LoadAggregateI32Indexed { source, .. }
        | Instruction::LoadAggregateU8Indexed { source, .. }
        | Instruction::LoadAggregateBoolIndexed { source, .. } => {
            matches!(source, AggregateLocation::Slot(_))
        }
        Instruction::TailCall { arguments, .. } => !arguments.is_empty(),
        Instruction::SetUsizeFromBorrow { .. } => true,
        Instruction::CheckFailure { failure_mode } => failure_mode_requires_frame(failure_mode),
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnOptionalNone
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::SetCurrentAllocationContext { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
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
        | Instruction::Trap
        | Instruction::Break
        | Instruction::Continue
        | Instruction::Return => false,
    }
}

pub(super) fn failure_mode_requires_frame(failure_mode: &FallibleFailureMode) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::PropagateWithCleanup { .. }
        | FallibleFailureMode::Handle { .. }
        | FallibleFailureMode::Recover { .. }
        | FallibleFailureMode::Catch { .. } => true,
    }
}
