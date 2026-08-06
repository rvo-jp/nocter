use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_instruction(
        &mut self,
        instruction: &Instruction,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match instruction {
            Instruction::WriteStr { fd, text } => {
                self.emit_write_str(fd, text, frame)?;
            }
            Instruction::WriteSlice { fd, bytes } => {
                self.emit_write_slice(fd, bytes, frame)?;
            }
            Instruction::ReadSlice {
                destination,
                fd,
                buffer,
                failure_mode,
            } => {
                self.emit_read_slice(*destination, fd, buffer, failure_mode, frame, return_type)?;
            }
            Instruction::OpenRead {
                destination,
                path,
                flags,
                mode,
                failure_mode,
            } => {
                self.emit_open_read(
                    *destination,
                    path,
                    flags,
                    mode,
                    failure_mode,
                    frame,
                    return_type,
                )?;
            }
            Instruction::CloseFd { fd } => {
                self.emit_close_fd(fd, frame)?;
            }
            Instruction::SetI32 { destination, value } => {
                self.emit_set_i32(*destination, value)?;
            }
            Instruction::SetU8 { destination, value } => {
                self.emit_set_u8(*destination, value)?;
            }
            Instruction::SetUsize { destination, value } => {
                self.emit_set_usize(*destination, value)?;
            }
            Instruction::RegionEnter { destination } => {
                self.emit_region_enter(*destination, frame)?;
            }
            Instruction::SetCurrentAllocationContext { state, kind } => {
                self.emit_set_current_allocation_context(state, kind)?;
            }
            Instruction::RegionRelease {
                state,
                parent_state,
                parent_kind,
            } => {
                self.emit_region_release(state, parent_state, parent_kind, frame)?;
            }
            Instruction::SetUsizeFromBorrow {
                destination,
                source,
            } => {
                self.emit_set_usize_from_borrow(*destination, *source, frame)?;
            }
            Instruction::SetBool { destination, value } => {
                self.emit_set_bool(*destination, value)?;
            }
            Instruction::SetStr { destination, value } => {
                self.emit_set_str(*destination, value)?;
            }
            Instruction::SetStrRawParts {
                destination,
                pointer,
                len,
            } => {
                self.emit_set_str_raw_parts(*destination, pointer, len)?;
            }
            Instruction::SetSlice { destination, value } => {
                self.emit_set_slice(*destination, value)?;
            }
            Instruction::SetSliceRawParts {
                destination,
                pointer,
                len,
            } => {
                self.emit_set_slice_raw_parts(*destination, pointer, len)?;
            }
            Instruction::ReserveAggregateSlot { .. } => {}
            Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_usize(*destination, *offset, value, frame)?;
            }
            Instruction::StoreAggregateUsizeIndexed {
                destination,
                base_offset,
                index,
                length,
                stride,
                value,
            } => {
                self.emit_store_aggregate_usize_indexed(
                    *destination,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    value,
                    frame,
                )?;
            }
            Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_i32(*destination, *offset, value, frame)?;
            }
            Instruction::StoreAggregateI32Indexed {
                destination,
                base_offset,
                index,
                length,
                stride,
                value,
            } => {
                self.emit_store_aggregate_i32_indexed(
                    *destination,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    value,
                    frame,
                )?;
            }
            Instruction::StoreAggregateU16 {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_u16(*destination, *offset, *value, frame)?;
            }
            Instruction::StoreAggregateU32 {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_u32(*destination, *offset, *value, frame)?;
            }
            Instruction::StoreAggregateU8 {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_u8(*destination, *offset, value, frame)?;
            }
            Instruction::StoreAggregateU8Indexed {
                destination,
                base_offset,
                index,
                length,
                stride,
                value,
            } => {
                self.emit_store_aggregate_u8_indexed(
                    *destination,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    value,
                    frame,
                )?;
            }
            Instruction::StoreAggregateBool {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_bool(*destination, *offset, value, frame)?;
            }
            Instruction::StoreAggregateBoolIndexed {
                destination,
                base_offset,
                index,
                length,
                stride,
                value,
            } => {
                self.emit_store_aggregate_bool_indexed(
                    *destination,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    value,
                    frame,
                )?;
            }
            Instruction::LoadAggregateUsize {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_usize(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateUsizeIndexed {
                destination,
                source,
                base_offset,
                index,
                length,
                stride,
            } => {
                self.emit_load_aggregate_usize_indexed(
                    *destination,
                    *source,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    frame,
                )?;
            }
            Instruction::LoadAggregateI32 {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_i32(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateI32Indexed {
                destination,
                source,
                base_offset,
                index,
                length,
                stride,
            } => {
                self.emit_load_aggregate_i32_indexed(
                    *destination,
                    *source,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    frame,
                )?;
            }
            Instruction::LoadAggregateU8 {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_u8(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateU8Indexed {
                destination,
                source,
                base_offset,
                index,
                length,
                stride,
            } => {
                self.emit_load_aggregate_u8_indexed(
                    *destination,
                    *source,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    frame,
                )?;
            }
            Instruction::LoadAggregateBool {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_bool(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateBoolIndexed {
                destination,
                source,
                base_offset,
                index,
                length,
                stride,
            } => {
                self.emit_load_aggregate_bool_indexed(
                    *destination,
                    *source,
                    *base_offset,
                    index,
                    *length,
                    *stride,
                    frame,
                )?;
            }
            Instruction::CopyAggregate {
                destination,
                source,
                layout,
            } => {
                self.emit_copy_aggregate(*destination, *source, *layout, frame)?;
            }
            Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source,
                source_offset,
                layout,
            } => {
                self.emit_copy_aggregate_range(
                    *destination,
                    *destination_offset,
                    *source,
                    *source_offset,
                    *layout,
                    frame,
                )?;
            }
            Instruction::CopySliceElementToAggregate {
                destination,
                source,
                index,
                layout,
            } => {
                self.emit_copy_slice_element_to_aggregate(
                    *destination,
                    *source,
                    *index,
                    *layout,
                    frame,
                )?;
            }
            Instruction::CopyAggregateToSliceElement {
                destination,
                index,
                source,
                layout,
            } => {
                self.emit_copy_aggregate_to_slice_element(
                    *destination,
                    *index,
                    *source,
                    *layout,
                    frame,
                )?;
            }
            Instruction::DarwinSyscall {
                destination,
                arity,
                number,
                arguments,
            } => {
                self.emit_darwin_syscall(*destination, *arity, number, arguments, frame)?;
            }
            Instruction::CopyStrToPointer {
                pointer,
                offset,
                text,
            } => {
                self.emit_copy_str_to_pointer(pointer, offset, text, frame)?;
            }
            Instruction::CopyPointerBytes {
                destination,
                source,
                byte_count,
            } => {
                self.emit_copy_pointer_bytes(destination, source, byte_count, frame)?;
            }
            Instruction::CopyAggregateToPointer {
                pointer,
                offset,
                source,
                layout,
            } => {
                self.emit_copy_aggregate_to_pointer(pointer, offset, *source, *layout, frame)?;
            }
            Instruction::CopyPointerToAggregate {
                destination,
                pointer,
                offset,
                layout,
            } => {
                self.emit_copy_pointer_to_aggregate(*destination, pointer, offset, *layout, frame)?;
            }
            Instruction::LoadU8FromPointer {
                destination,
                pointer,
                offset,
            } => self.emit_load_u8_from_pointer(*destination, pointer, offset)?,
            Instruction::LoadI32FromPointer {
                destination,
                pointer,
                offset,
            } => self.emit_load_i32_from_pointer(*destination, pointer, offset)?,
            Instruction::LoadUsizeFromPointer {
                destination,
                pointer,
                offset,
            } => self.emit_load_usize_from_pointer(*destination, pointer, offset)?,
            Instruction::LoadBoolFromPointer {
                destination,
                pointer,
                offset,
            } => self.emit_load_bool_from_pointer(*destination, pointer, offset)?,
            Instruction::LoadStrFromPointer {
                destination,
                pointer,
                offset,
            } => self.emit_load_str_from_pointer(*destination, pointer, offset)?,
            Instruction::StoreU8ToPointer {
                pointer,
                offset,
                value,
            } => {
                self.emit_store_u8_to_pointer(pointer, offset, value, frame)?;
            }
            Instruction::StoreI32ToPointer {
                pointer,
                offset,
                value,
            } => {
                self.emit_store_i32_to_pointer(pointer, offset, value, frame)?;
            }
            Instruction::StoreUsizeToPointer {
                pointer,
                offset,
                value,
            } => {
                self.emit_store_usize_to_pointer(pointer, offset, value, frame)?;
            }
            Instruction::StoreBoolToPointer {
                pointer,
                offset,
                value,
            } => {
                self.emit_store_bool_to_pointer(pointer, offset, value, frame)?;
            }
            Instruction::StoreStrToPointer {
                pointer,
                offset,
                value,
            } => {
                self.emit_store_str_to_pointer(pointer, offset, value, frame)?;
            }
            Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value,
            } => {
                self.emit_store_u8_to_slice_index(*destination, index, value, frame)?;
            }
            Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value,
            } => {
                self.emit_store_i32_to_slice_index(*destination, index, value, frame)?;
            }
            Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value,
            } => {
                self.emit_store_usize_to_slice_index(*destination, index, value, frame)?;
            }
            Instruction::StoreBoolToSliceIndex {
                destination,
                index,
                value,
            } => {
                self.emit_store_bool_to_slice_index(*destination, index, value, frame)?;
            }
            Instruction::StoreStrToSliceIndex {
                destination,
                index,
                value,
            } => {
                self.emit_store_str_to_slice_index(*destination, index, value, frame)?;
            }
            Instruction::AddU8 {
                destination,
                left,
                right,
            } => {
                self.emit_add_u8(*destination, left, right)?;
            }
            Instruction::SubtractU8 {
                destination,
                left,
                right,
            } => {
                self.emit_subtract_u8(*destination, left, right)?;
            }
            Instruction::MultiplyU8 {
                destination,
                left,
                right,
            } => {
                self.emit_multiply_u8(*destination, left, right)?;
            }
            Instruction::DivideU8 {
                destination,
                left,
                right,
            } => {
                self.emit_divide_u8(*destination, left, right)?;
            }
            Instruction::RemainderU8 {
                destination,
                left,
                right,
            } => {
                self.emit_remainder_u8(*destination, left, right)?;
            }
            Instruction::ShiftLeftU8 {
                destination,
                left,
                right,
            } => {
                self.emit_shift_left_u8(*destination, left, right)?;
            }
            Instruction::ShiftRightU8 {
                destination,
                left,
                right,
            } => {
                self.emit_shift_right_u8(*destination, left, right)?;
            }
            Instruction::AddI32 {
                destination,
                left,
                right,
            } => {
                self.emit_add_i32(*destination, left, right)?;
            }
            Instruction::SubtractI32 {
                destination,
                left,
                right,
            } => {
                self.emit_subtract_i32(*destination, left, right)?;
            }
            Instruction::MultiplyI32 {
                destination,
                left,
                right,
            } => {
                self.emit_multiply_i32(*destination, left, right)?;
            }
            Instruction::DivideI32 {
                destination,
                left,
                right,
            } => {
                self.emit_divide_i32(*destination, left, right)?;
            }
            Instruction::RemainderI32 {
                destination,
                left,
                right,
            } => {
                self.emit_remainder_i32(*destination, left, right)?;
            }
            Instruction::ShiftLeftI32 {
                destination,
                left,
                right,
            } => {
                self.emit_shift_left_i32(*destination, left, right)?;
            }
            Instruction::ShiftRightI32 {
                destination,
                left,
                right,
            } => {
                self.emit_shift_right_i32(*destination, left, right)?;
            }
            Instruction::AddUsize {
                destination,
                left,
                right,
            } => {
                self.emit_add_usize(*destination, left, right)?;
            }
            Instruction::SubtractUsize {
                destination,
                left,
                right,
            } => {
                self.emit_subtract_usize(*destination, left, right)?;
            }
            Instruction::MultiplyUsize {
                destination,
                left,
                right,
            } => {
                self.emit_multiply_usize(*destination, left, right)?;
            }
            Instruction::DivideUsize {
                destination,
                left,
                right,
            } => {
                self.emit_divide_usize(*destination, left, right)?;
            }
            Instruction::RemainderUsize {
                destination,
                left,
                right,
            } => {
                self.emit_remainder_usize(*destination, left, right)?;
            }
            Instruction::ShiftLeftUsize {
                destination,
                left,
                right,
            } => {
                self.emit_shift_left_usize(*destination, left, right)?;
            }
            Instruction::ShiftRightUsize {
                destination,
                left,
                right,
            } => {
                self.emit_shift_right_usize(*destination, left, right)?;
            }
            Instruction::CallI32 {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_i32(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeI32 {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_i32(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallU8 {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_u8(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeU8 {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_u8(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallUsize {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_usize(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeUsize {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_usize(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallBorrow {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_usize(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeBorrow {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_usize(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallBool {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_bool(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeBool {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_bool(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallStr {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_str(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeStr {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_str(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallSlice {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_slice(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallOutcomeSlice {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_slice(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallAggregate {
                destination,
                target,
                arguments,
            } => {
                self.emit_call_aggregate(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                )?;
            }
            Instruction::CallDirectAggregate {
                destination,
                target,
                arguments,
                layout,
            } => {
                self.emit_call_direct_aggregate(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    *layout,
                    frame,
                )?;
            }
            Instruction::CallOutcomeDirectAggregate {
                destination,
                target,
                arguments,
                layout,
                failure_mode,
            } => {
                self.emit_call_fallible_direct_aggregate(
                    calls::OutcomeDirectAggregateCall {
                        destination: *destination,
                        function: FunctionSymbol::from_call_target(target),
                        arguments,
                        layout: *layout,
                    },
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallOutcomeAggregate {
                destination,
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_aggregate(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallVoid { target, arguments } => {
                self.emit_call_void(FunctionSymbol::from_call_target(target), arguments, frame)?;
            }
            Instruction::CallOutcomeVoid {
                target,
                arguments,
                failure_mode,
            } => {
                self.emit_call_fallible_void(
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    frame,
                    failure_mode,
                    return_type,
                )?;
            }
            Instruction::CallComposedOutcome {
                destination,
                target,
                arguments,
                outer,
                inner,
                outer_mode,
                inner_mode,
            } => {
                self.emit_call_composed_outcome(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    *outer,
                    *inner,
                    outer_mode,
                    inner_mode,
                    frame,
                    return_type,
                )?;
            }
            Instruction::CallStoredOutcome {
                destination,
                target,
                arguments,
                storage,
                payload_type,
            } => {
                self.emit_call_stored_outcome(
                    *destination,
                    FunctionSymbol::from_call_target(target),
                    arguments,
                    storage,
                    payload_type,
                    frame,
                )?;
            }
            Instruction::IfStoredOutcomeTag {
                source,
                tag_offset,
                success_instructions,
                outcome_instructions,
            } => {
                self.emit_if_stored_outcome_tag(
                    *source,
                    *tag_offset,
                    success_instructions,
                    outcome_instructions,
                    frame,
                    return_type,
                )?;
            }
            Instruction::CheckStoredFallible {
                source,
                tag_offset,
                error_offset,
                success_instructions,
                failure_mode,
            } => {
                self.emit_check_stored_fallible(
                    *source,
                    *tag_offset,
                    *error_offset,
                    success_instructions,
                    failure_mode,
                    frame,
                    return_type,
                )?;
            }
            Instruction::LoadStoredOutcomePayload {
                destination,
                source,
                offset,
            } => {
                self.emit_load_stored_outcome_payload(*destination, *source, *offset, frame)?;
            }
            Instruction::ReturnStoredOutcome {
                source,
                storage,
                payload_type,
            } => {
                self.emit_return_stored_outcome(*source, storage, payload_type, frame)?;
            }
            Instruction::TailCall { target, arguments } => {
                self.emit_tail_call(FunctionSymbol::from_call_target(target), arguments, frame)?;
            }
            Instruction::ProcessExit { code } => {
                self.emit_process_exit(code)?;
            }
            Instruction::Trap => {
                self.emit_trap();
            }
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => {
                self.emit_if(
                    condition,
                    then_instructions,
                    else_instructions,
                    frame,
                    return_type,
                )?;
            }
            Instruction::While {
                condition_instructions,
                condition,
                body_instructions,
            } => {
                self.emit_while(
                    condition_instructions,
                    condition,
                    body_instructions,
                    frame,
                    return_type,
                )?;
            }
            Instruction::Break => {
                self.emit_break()?;
            }
            Instruction::Continue => {
                self.emit_continue()?;
            }
            Instruction::PropagateFailure => {
                self.emit_propagate_failure(frame)?;
            }
            Instruction::TrapOnFailure => {
                self.emit_trap_on_failure()?;
            }
            Instruction::CheckFailure { failure_mode } => {
                self.emit_check_failure(failure_mode, frame, return_type)?;
            }
            Instruction::ReturnOutcomeSuccess => {
                self.emit_return_outcome_success(return_type, frame)?;
            }
            Instruction::ReturnOptionalNone => {
                self.emit_return_optional_none(frame, return_type)?;
            }
            Instruction::ReturnFallibleFailure { code, message } => {
                self.emit_return_fallible_failure(code, message, frame, return_type)?;
            }
            Instruction::Return => {
                self.emit_return(frame);
            }
        }

        Ok(())
    }
}
