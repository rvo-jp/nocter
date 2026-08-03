use super::*;

pub(super) fn aggregate_slot_requests(
    instructions: &[Instruction],
) -> Result<Vec<AggregateSlotRequest>, Vec<Diagnostic>> {
    let mut requests = Vec::new();
    record_instruction_list_aggregate_slot_requests(instructions, &mut requests)?;
    Ok(requests)
}

pub(super) fn record_instruction_list_aggregate_slot_requests(
    instructions: &[Instruction],
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    for instruction in instructions {
        record_instruction_aggregate_slot_requests(instruction, requests)?;
    }

    Ok(())
}

pub(super) fn record_instruction_aggregate_slot_requests(
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
        Instruction::CopyAggregateToPointer { source, layout, .. } => {
            if let AggregateLocation::Slot(slot_index) = source {
                record_aggregate_slot_request(*slot_index, *layout, requests)?;
            }
            Ok(())
        }
        Instruction::CopyPointerToAggregate {
            destination,
            layout,
            ..
        } => {
            if let AggregateLocation::Slot(slot_index) = destination {
                record_aggregate_slot_request(*slot_index, *layout, requests)?;
            }
            Ok(())
        }
        Instruction::CopySliceElementToAggregate {
            destination,
            layout,
            ..
        } => {
            if let AggregateLocation::Slot(slot_index) = destination {
                record_aggregate_slot_request(*slot_index, *layout, requests)?;
            }
            Ok(())
        }
        Instruction::CopyAggregateToSliceElement { source, layout, .. } => {
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
        | Instruction::CallFallibleBorrow { failure_mode, .. }
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
        Instruction::CallComposedOutcome {
            outer_mode,
            inner_mode,
            ..
        } => {
            record_failure_mode_aggregate_slot_requests(outer_mode, requests)?;
            record_failure_mode_aggregate_slot_requests(inner_mode, requests)
        }
        Instruction::PropagateFailure
        | Instruction::TrapOnFailure
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnOptionalNone
        | Instruction::ReturnFallibleFailure { .. }
        | Instruction::ProcessExit { .. }
        | Instruction::WriteStr { .. }
        | Instruction::WriteSlice { .. }
        | Instruction::CallBorrow { .. }
        | Instruction::CloseFd { .. }
        | Instruction::DarwinSyscall { .. }
        | Instruction::CopyStrToPointer { .. }
        | Instruction::CopyPointerBytes { .. }
        | Instruction::LoadU8FromPointer { .. }
        | Instruction::LoadI32FromPointer { .. }
        | Instruction::LoadUsizeFromPointer { .. }
        | Instruction::LoadBoolFromPointer { .. }
        | Instruction::LoadStrFromPointer { .. }
        | Instruction::StoreU8ToPointer { .. }
        | Instruction::StoreI32ToPointer { .. }
        | Instruction::StoreUsizeToPointer { .. }
        | Instruction::StoreBoolToPointer { .. }
        | Instruction::StoreStrToPointer { .. }
        | Instruction::StoreU8ToSliceIndex { .. }
        | Instruction::StoreI32ToSliceIndex { .. }
        | Instruction::StoreUsizeToSliceIndex { .. }
        | Instruction::StoreBoolToSliceIndex { .. }
        | Instruction::StoreStrToSliceIndex { .. }
        | Instruction::SetI32 { .. }
        | Instruction::SetStrRawParts { .. }
        | Instruction::SetSliceRawParts { .. }
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
        | Instruction::SetU8 { .. }
        | Instruction::SetUsize { .. }
        | Instruction::RegionEnter { .. }
        | Instruction::SetCurrentAllocationContext { .. }
        | Instruction::RegionRelease { .. }
        | Instruction::SetUsizeFromBorrow { .. }
        | Instruction::SetBool { .. }
        | Instruction::SetStr { .. }
        | Instruction::SetSlice { .. }
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

pub(super) fn record_failure_mode_aggregate_slot_requests(
    failure_mode: &FallibleFailureMode,
    requests: &mut Vec<AggregateSlotRequest>,
) -> Result<(), Vec<Diagnostic>> {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => Ok(()),
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            record_instruction_list_aggregate_slot_requests(instructions, requests)
        }
    }
}

pub(super) fn record_aggregate_slot_request(
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

pub(super) fn aggregate_slots(
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
