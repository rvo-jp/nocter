use crate::abi::ReturnPassing;
use crate::backend::frame::{FrameLayout, FunctionFrame, plan_function_frame};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::{
    BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, Function, I32Location,
    I32Value, Instruction, IrModule, ScalarArgument, SliceValue, StrLocation, StrValue, Type,
    UsizeLocation, UsizeValue,
};
use crate::target::arm64::{BranchCondition, Encoder, MoveWideShift, WReg, XReg};
use std::collections::HashMap;

mod abi_shapes;
mod calls;
mod control_flow;
mod emission;
mod locations;
mod process_arguments;
mod runtime;
mod symbols;
mod validation;
mod values;

use abi_shapes::*;
use emission::*;
use process_arguments::*;
use runtime::*;
use symbols::*;
use validation::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineCode {
    pub(crate) text: Vec<u8>,
    pub(crate) read_only_data: Vec<u8>,
}

pub(crate) fn generate_arm64_darwin_entry(
    module: &IrModule,
) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut emitter = EntryEmitter::new();
    emitter.emit_module(module)?;
    emitter.finish()
}

#[derive(Debug, Default)]
struct EntryEmitter {
    encoder: Encoder,
    read_only_data: Vec<u8>,
    data_address_patches: Vec<DataAddressPatch>,
    function_offsets: HashMap<FunctionSymbol, usize>,
    call_patches: Vec<FunctionCallPatch>,
    tail_call_patches: Vec<FunctionCallPatch>,
    loop_contexts: Vec<LoopContext>,
    current_frame_size: Option<u32>,
    current_parameter_spill_offsets: HashMap<usize, u32>,
    current_scalar_spill_offsets: HashMap<usize, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopContext {
    start_offset: usize,
    break_branches: Vec<control_flow::BranchPatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StaticErrorPayload {
    code: &'static [u8],
    message: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DarwinErrnoPayload {
    errno: i32,
    payload: StaticErrorPayload,
}

impl EntryEmitter {
    fn new() -> Self {
        Self {
            encoder: Encoder::new(),
            read_only_data: Vec::new(),
            data_address_patches: Vec::new(),
            function_offsets: HashMap::new(),
            call_patches: Vec::new(),
            tail_call_patches: Vec::new(),
            loop_contexts: Vec::new(),
            current_frame_size: None,
            current_parameter_spill_offsets: HashMap::new(),
            current_scalar_spill_offsets: HashMap::new(),
        }
    }

    fn emit_module(&mut self, module: &IrModule) -> Result<(), Vec<Diagnostic>> {
        let Some(entry) = module
            .functions
            .iter()
            .find(|function| function.name == DEFAULT_ENTRY_NAME)
        else {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "codegen requires a lowered entry function `{}`",
                    DEFAULT_ENTRY_NAME
                ),
            )]);
        };
        validate_module_call_return_shapes(module)?;

        self.emit_process_entry(entry, module_uses_process_arguments(module))?;

        for function in &module.functions {
            self.emit_function(function)?;
        }

        Ok(())
    }

    fn emit_process_entry(
        &mut self,
        entry: &Function,
        capture_process_stack: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        if capture_process_stack {
            self.encoder.emit_add_x_sp_imm(XReg::X19, 0);
        }
        self.emit_call(FunctionSymbol::from_function(entry));

        if let Type::Fallible(success_type) = &entry.return_type {
            self.emit_fallible_process_exit(success_type)?;
            return Ok(());
        }

        if matches!(entry.return_type, Type::Void) {
            emit_mov_i32_to_w0(&mut self.encoder, 0);
        }
        emit_darwin_exit_syscall(&mut self.encoder);
        Ok(())
    }

    fn emit_function(&mut self, function: &Function) -> Result<(), Vec<Diagnostic>> {
        self.function_offsets.insert(
            FunctionSymbol::from_function(function),
            self.encoder.position(),
        );
        let frame = plan_function_frame(function)?;
        self.emit_function_with_frame(function, &frame)
    }

    fn emit_function_with_frame(
        &mut self,
        function: &Function,
        frame: &FunctionFrame,
    ) -> Result<(), Vec<Diagnostic>> {
        let previous_frame_size = self.current_frame_size;
        let previous_parameter_spill_offsets =
            std::mem::take(&mut self.current_parameter_spill_offsets);
        let previous_scalar_spill_offsets = std::mem::take(&mut self.current_scalar_spill_offsets);
        let frame = match frame {
            FunctionFrame::Frameless => {
                self.current_frame_size = Some(0);
                None
            }
            FunctionFrame::Framed(layout) => {
                self.current_frame_size = Some(layout.frame_size());
                self.current_parameter_spill_offsets = layout
                    .parameter_spill_slots()
                    .iter()
                    .map(|slot| (slot.parameter_index(), slot.offset()))
                    .collect();
                self.current_scalar_spill_offsets = layout
                    .scalar_spill_slots()
                    .iter()
                    .map(|slot| (slot.local_index(), slot.offset()))
                    .collect();
                self.emit_prologue(layout)?;
                Some(layout)
            }
        };

        let result = (|| {
            for instruction in &function.instructions {
                self.emit_instruction(instruction, frame, &function.return_type)?;
            }
            Ok(())
        })();
        self.current_frame_size = previous_frame_size;
        self.current_parameter_spill_offsets = previous_parameter_spill_offsets;
        self.current_scalar_spill_offsets = previous_scalar_spill_offsets;
        result
    }

    fn emit_instruction(
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
                failure_mode,
            } => {
                self.emit_open_read(*destination, path, failure_mode, frame, return_type)?;
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
            Instruction::CallFallibleI32 {
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
            Instruction::CallFallibleU8 {
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
            Instruction::CallFallibleUsize {
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
            Instruction::CallFallibleBool {
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
            Instruction::CallFallibleStr {
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
            Instruction::CallFallibleSlice {
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
            Instruction::CallFallibleDirectAggregate {
                destination,
                target,
                arguments,
                layout,
                failure_mode,
            } => {
                self.emit_call_fallible_direct_aggregate(
                    calls::FallibleDirectAggregateCall {
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
            Instruction::CallFallibleAggregate {
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
            Instruction::CallFallibleVoid {
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
            Instruction::ReturnFallibleSuccess => {
                self.emit_return_fallible_success(return_type, frame)?;
            }
            Instruction::ReturnOptionalNone => {
                self.emit_return_optional_none(frame);
            }
            Instruction::ReturnFallibleFailure { code, message } => {
                self.emit_return_fallible_failure(code, message, frame)?;
            }
            Instruction::Return => {
                self.emit_return(frame);
            }
        }

        Ok(())
    }

    fn emit_process_exit(&mut self, code: &I32Value) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_value_to_w(code, WReg::W0)?;
        emit_darwin_exit_syscall(&mut self.encoder);
        Ok(())
    }

    fn emit_fallible_process_exit(&mut self, success_type: &Type) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Ne);

        match success_type {
            Type::I32 => {
                self.encoder.emit_mov_w(WReg::W0, WReg::W1);
            }
            Type::Usize => {
                self.encoder.emit_mov_x(XReg::X0, XReg::X1);
            }
            Type::Void => {
                emit_mov_i32_to_w0(&mut self.encoder, 0);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "codegen only supports `i32!`, `usize!`, and `void!` executable entry returns",
                )]);
            }
        }
        emit_darwin_exit_syscall(&mut self.encoder);

        self.patch_branch_placeholder_to_current(failure_branch, "fallible entry failure target")?;
        self.emit_fallible_entry_failure_report();
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        emit_darwin_exit_syscall(&mut self.encoder);
        Ok(())
    }

    fn emit_fallible_entry_failure_report(&mut self) {
        self.encoder.emit_sub_sp_imm(FALLIBLE_REPORT_FRAME_SIZE);
        self.encoder.emit_str_x_sp(XReg::X1, 0);
        self.encoder.emit_str_x_sp(XReg::X2, 8);
        self.encoder.emit_str_x_sp(XReg::X3, 16);
        self.encoder.emit_str_x_sp(XReg::X4, 24);

        self.emit_stack_str_to_stderr(0, 8);
        self.emit_write_static_stderr(b": ");
        self.emit_stack_str_to_stderr(16, 24);
        self.emit_write_static_stderr(b"\n");

        self.encoder.emit_add_sp_imm(FALLIBLE_REPORT_FRAME_SIZE);
    }

    fn emit_stack_str_to_stderr(&mut self, ptr_offset: u32, len_offset: u32) {
        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        self.encoder.emit_ldr_x_sp(XReg::X1, ptr_offset);
        self.encoder.emit_ldr_x_sp(XReg::X2, len_offset);
        emit_darwin_write_syscall(&mut self.encoder);
    }

    fn emit_return_fallible_success(
        &mut self,
        return_type: &Type,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Type::Fallible(success_type) = return_type else {
            return Err(vec![Diagnostic::error(
                "E9002",
                "`ReturnFallibleSuccess` requires a fallible function return type",
            )]);
        };

        self.emit_fallible_success_payload(success_type)?;
        emit_mov_i32_to_w0(&mut self.encoder, 0);
        self.emit_return(frame);
        Ok(())
    }

    fn emit_fallible_success_payload(
        &mut self,
        success_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_supported_fallible_success_payload_abi(success_type)?;
        match success_type {
            Type::I32 | Type::U8 | Type::Bool => {
                self.encoder.emit_mov_w(WReg::W1, WReg::W0);
            }
            Type::Usize => {
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Str | Type::Slice { .. } => {
                self.encoder.emit_mov_x(XReg::X2, XReg::X1);
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Aggregate { .. } => {}
            Type::DirectAggregate { words, .. } => match words {
                0 => {}
                1 => {
                    self.encoder.emit_mov_x(XReg::X1, XReg::X0);
                }
                2 => {
                    self.encoder.emit_mov_x(XReg::X2, XReg::X1);
                    self.encoder.emit_mov_x(XReg::X1, XReg::X0);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "E9002",
                        "invalid direct aggregate fallible success payload width",
                    )]);
                }
            },
            Type::Void => {}
            Type::Borrow { .. } | Type::Error | Type::Never | Type::Fallible(_) => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "invalid fallible success payload type for codegen",
                )]);
            }
        }

        Ok(())
    }

    fn emit_return_fallible_failure(
        &mut self,
        code: &StrValue,
        message: &StrValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_failure_payload_to_registers(code, message)?;
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        self.emit_return(frame);
        Ok(())
    }

    fn emit_failure_payload_to_registers(
        &mut self,
        code: &StrValue,
        message: &StrValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let code_sources = self.str_value_source_registers(code)?;
        let message_sources = self.str_value_source_registers(message)?;
        let code_destinations = [XReg::X1, XReg::X2];
        let message_destinations = [XReg::X3, XReg::X4];

        let code_clobbers_message = registers_overlap(&code_destinations, &message_sources);
        let message_clobbers_code = registers_overlap(&message_destinations, &code_sources);

        match (code_clobbers_message, message_clobbers_code) {
            (true, true) => {
                let (temporary_ptr, temporary_len) =
                    failure_payload_temporary_pair(&message_sources, &message_destinations)?;
                self.emit_str_value_to_x_pair(code, temporary_ptr, temporary_len)?;
                self.emit_str_value_to_x_pair(message, XReg::X3, XReg::X4)?;
                self.emit_x_pair_to_x_pair(temporary_ptr, temporary_len, XReg::X1, XReg::X2)?;
            }
            (true, false) => {
                self.emit_str_value_to_x_pair(message, XReg::X3, XReg::X4)?;
                self.emit_str_value_to_x_pair(code, XReg::X1, XReg::X2)?;
            }
            _ => {
                self.emit_str_value_to_x_pair(code, XReg::X1, XReg::X2)?;
                self.emit_str_value_to_x_pair(message, XReg::X3, XReg::X4)?;
            }
        }
        Ok(())
    }

    fn str_value_source_registers(
        &self,
        value: &StrValue,
    ) -> Result<[Option<XReg>; 2], Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(_) => Ok([None, None]),
            StrValue::Location(location) => self.str_location_source_registers(*location),
            StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. } => Ok([None, None]),
        }
    }

    fn str_location_source_registers(
        &self,
        location: StrLocation,
    ) -> Result<[Option<XReg>; 2], Vec<Diagnostic>> {
        match location {
            StrLocation::Return => Ok([Some(XReg::X0), Some(XReg::X1)]),
            StrLocation::Parameter(index) => {
                let len_index = checked_pair_len_index(index, "parameter failure payload")?;
                Ok([
                    self.parameter_word_source_register(index),
                    self.parameter_word_source_register(len_index),
                ])
            }
            StrLocation::Local(index) => {
                let len_index = checked_pair_len_index(index, "local failure payload")?;
                Ok([
                    self.local_word_source_register(index),
                    self.local_word_source_register(len_index),
                ])
            }
        }
    }

    fn parameter_word_source_register(&self, index: usize) -> Option<XReg> {
        if self.current_parameter_spill_offsets.contains_key(&index) {
            return None;
        }
        XReg::argument(index)
    }

    fn local_word_source_register(&self, index: usize) -> Option<XReg> {
        XReg::local(index)
    }

    fn emit_return_optional_none(&mut self, frame: Option<&FrameLayout>) {
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        self.emit_return(frame);
    }

    fn emit_propagate_failure(
        &mut self,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_return(frame);
        self.patch_branch_placeholder_to_current(success_branch, "fallible success target")
    }

    fn emit_trap_on_failure(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(success_branch, "fallible force success target")
    }

    fn emit_check_failure(
        &mut self,
        failure_mode: &FallibleFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        match failure_mode {
            FallibleFailureMode::Propagate => self.emit_return(frame),
            FallibleFailureMode::PropagateWithCleanup { .. }
            | FallibleFailureMode::Handle { .. }
            | FallibleFailureMode::Recover { .. } => {
                let Some(frame) = frame else {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "fallible failure handler emission requires a stack frame",
                    )]);
                };
                self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
            }
            FallibleFailureMode::Trap => self.emit_trap(),
            FallibleFailureMode::Catch { .. } => {
                let Some(frame) = frame else {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "catch failure emission requires a stack frame",
                    )]);
                };
                self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
            }
        }
        self.patch_branch_placeholder_to_current(success_branch, "fallible success target")
    }

    fn emit_fallible_failure_action(
        &mut self,
        failure_mode: &FallibleFailureMode,
        frame: &FrameLayout,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match failure_mode {
            FallibleFailureMode::Propagate => {
                self.emit_return(Some(frame));
                Ok(())
            }
            FallibleFailureMode::PropagateWithCleanup {
                code,
                message,
                instructions,
            } => {
                self.emit_scalar_reloads(frame)?;
                self.emit_x_pair_to_str_location(XReg::X1, XReg::X2, *code)?;
                self.emit_x_pair_to_str_location(XReg::X3, XReg::X4, *message)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                self.emit_str_value_to_x_pair(&StrValue::Location(*code), XReg::X1, XReg::X2)?;
                self.emit_str_value_to_x_pair(&StrValue::Location(*message), XReg::X3, XReg::X4)?;
                emit_mov_i32_to_w0(&mut self.encoder, 1);
                self.emit_return(Some(frame));
                Ok(())
            }
            FallibleFailureMode::Trap => {
                self.emit_trap();
                Ok(())
            }
            FallibleFailureMode::Handle { instructions } => {
                self.emit_scalar_reloads(frame)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            FallibleFailureMode::Recover { instructions } => {
                self.emit_scalar_reloads(frame)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            FallibleFailureMode::Catch {
                code,
                message,
                instructions,
            } => {
                self.emit_scalar_reloads(frame)?;
                self.emit_x_pair_to_str_location(XReg::X1, XReg::X2, *code)?;
                self.emit_x_pair_to_str_location(XReg::X3, XReg::X4, *message)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
        }
    }

    fn emit_prologue(&mut self, frame: &FrameLayout) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_sub_sp_imm(frame.frame_size());
        self.encoder
            .emit_str_x_sp(XReg::X30, frame.saved_x30_offset());
        if let Some(offset) = frame.indirect_return_pointer_offset() {
            self.encoder.emit_str_x_sp(XReg::X8, offset);
        }
        for slot in frame.parameter_spill_slots() {
            self.emit_unspilled_parameter_word_to_x(slot.parameter_index(), XReg::X16)?;
            self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
        }
        Ok(())
    }

    fn emit_epilogue(&mut self, frame: &FrameLayout) {
        self.encoder
            .emit_ldr_x_sp(XReg::X30, frame.saved_x30_offset());
        self.encoder.emit_add_sp_imm(frame.frame_size());
    }

    fn emit_return(&mut self, frame: Option<&FrameLayout>) {
        if let Some(frame) = frame {
            self.emit_epilogue(frame);
        }
        self.encoder.emit_ret();
    }

    fn emit_static_error_payload(
        &mut self,
        payload: StaticErrorPayload,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(payload.code.to_vec()),
            XReg::X1,
            XReg::X2,
        )?;
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(payload.message.to_vec()),
            XReg::X3,
            XReg::X4,
        )?;
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        Ok(())
    }

    fn emit_error_payload_from_errno(
        &mut self,
        mappings: &[DarwinErrnoPayload],
        fallback: StaticErrorPayload,
        done_target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut done_branches = Vec::new();

        for mapping in mappings {
            emit_mov_i32_to_w(&mut self.encoder, WReg::W17, mapping.errno);
            self.encoder.emit_cmp_w(WReg::W0, WReg::W17);
            let next_mapping = self.emit_cond_branch_placeholder(BranchCondition::Ne);
            self.emit_static_error_payload(mapping.payload)?;
            done_branches.push(self.emit_branch_placeholder());
            self.patch_branch_placeholder_to_current(
                next_mapping,
                "errno error payload next mapping target",
            )?;
        }

        self.emit_static_error_payload(fallback)?;
        self.patch_branch_placeholders_to_current(done_branches, done_target_description)
    }

    fn emit_open_failure_payload_from_errno(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            OPEN_ERRNO_PAYLOADS,
            OPEN_FAILURE_PAYLOAD,
            "open failure payload end target",
        )
    }

    fn emit_read_failure_payload_from_errno(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            READ_ERRNO_PAYLOADS,
            READ_FAILURE_PAYLOAD,
            "read failure payload end target",
        )
    }

    fn emit_write_failure_payload_from_errno(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            WRITE_ERRNO_PAYLOADS,
            WRITE_FAILURE_PAYLOAD,
            "write failure payload end target",
        )
    }

    pub(super) fn emit_indirect_return_pointer_to_x8(&mut self, frame: Option<&FrameLayout>) {
        if let Some(offset) = frame.and_then(FrameLayout::indirect_return_pointer_offset) {
            self.encoder.emit_ldr_x_sp(XReg::X8, offset);
        }
    }

    fn emit_write_static_stderr(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        self.emit_static_data_address(XReg::X1, bytes);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, bytes.len() as u64);
        emit_darwin_write_syscall(&mut self.encoder);
    }

    fn emit_write_str(
        &mut self,
        fd: &I32Value,
        text: &StrValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "str write emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_str_value_to_x_pair(text, XReg::X3, XReg::X4)?;
        self.emit_i32_value_to_w(fd, WReg::W0)?;
        self.emit_write_all_syscall_loop()?;
        self.emit_scalar_reloads(frame)?;
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_write_failure_payload_from_errno()?;
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(success_branch, "write syscall success target")?;
        emit_mov_i32_to_w0(&mut self.encoder, 0);

        self.patch_branch_placeholder_to_current(end_branch, "write syscall end target")?;
        Ok(())
    }

    fn emit_write_slice(
        &mut self,
        fd: &I32Value,
        bytes: &SliceValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice write emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_slice_value_to_x_pair(bytes, XReg::X3, XReg::X4)?;
        self.emit_i32_value_to_w(fd, WReg::W0)?;
        self.emit_write_all_syscall_loop()?;
        self.emit_scalar_reloads(frame)?;
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_write_failure_payload_from_errno()?;
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(success_branch, "write syscall success target")?;
        emit_mov_i32_to_w0(&mut self.encoder, 0);

        self.patch_branch_placeholder_to_current(end_branch, "write syscall end target")?;
        Ok(())
    }

    fn emit_write_all_syscall_loop(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_sub_sp_imm(WRITE_LOOP_FRAME_SIZE);
        self.encoder.emit_sxtw_x_w(XReg::X5, WReg::W0);
        self.encoder.emit_str_x_sp(XReg::X5, WRITE_LOOP_FD_OFFSET);
        self.encoder
            .emit_str_x_sp(XReg::X3, WRITE_LOOP_POINTER_OFFSET);
        self.encoder
            .emit_str_x_sp(XReg::X4, WRITE_LOOP_REMAINING_OFFSET);

        let loop_start_offset = self.encoder.position();
        self.encoder
            .emit_ldr_x_sp(XReg::X2, WRITE_LOOP_REMAINING_OFFSET);
        self.encoder.emit_cmp_x_zero(XReg::X2);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);

        self.encoder.emit_ldr_x_sp(XReg::X0, WRITE_LOOP_FD_OFFSET);
        self.encoder
            .emit_ldr_x_sp(XReg::X1, WRITE_LOOP_POINTER_OFFSET);
        emit_darwin_write_syscall(&mut self.encoder);
        let syscall_failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Cs);

        self.encoder.emit_cmp_x_zero(XReg::X0);
        let zero_write_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder
            .emit_ldr_x_sp(XReg::X2, WRITE_LOOP_REMAINING_OFFSET);
        self.encoder.emit_cmp_x(XReg::X2, XReg::X0);
        let count_in_range_branch = self.emit_cond_branch_placeholder(BranchCondition::Cs);

        self.patch_branch_placeholder_to_current(
            zero_write_branch,
            "write syscall zero-byte failure target",
        )?;
        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, WRITE_UNEXPECTED_RESULT_ERRNO);
        let unexpected_count_failure_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(
            count_in_range_branch,
            "write syscall partial-progress target",
        )?;
        self.encoder
            .emit_ldr_x_sp(XReg::X1, WRITE_LOOP_POINTER_OFFSET);
        self.encoder.emit_adds_x(XReg::X1, XReg::X1, XReg::X0);
        self.encoder
            .emit_str_x_sp(XReg::X1, WRITE_LOOP_POINTER_OFFSET);
        self.encoder.emit_subs_x(XReg::X2, XReg::X2, XReg::X0);
        self.encoder
            .emit_str_x_sp(XReg::X2, WRITE_LOOP_REMAINING_OFFSET);
        let loop_branch = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            loop_branch,
            loop_start_offset,
            "write syscall loop target",
        )?;

        self.patch_branch_placeholder_to_current(success_branch, "write syscall done target")?;
        self.encoder.emit_add_sp_imm(WRITE_LOOP_FRAME_SIZE);
        emit_mov_i32_to_w0(&mut self.encoder, 0);
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(
            syscall_failure_branch,
            "write syscall failure target",
        )?;
        self.patch_branch_placeholder_to_current(
            unexpected_count_failure_branch,
            "write syscall unexpected-count failure target",
        )?;
        self.encoder.emit_add_sp_imm(WRITE_LOOP_FRAME_SIZE);

        self.patch_branch_placeholder_to_current(end_branch, "write syscall result target")
    }

    fn emit_read_slice(
        &mut self,
        destination: UsizeLocation,
        fd: &I32Value,
        buffer: &SliceValue,
        failure_mode: &FallibleFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice read emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_slice_value_to_x_pair(buffer, XReg::X3, XReg::X4)?;
        self.emit_i32_value_to_w(fd, WReg::W0)?;
        self.encoder.emit_mov_x(XReg::X1, XReg::X3);
        self.encoder.emit_mov_x(XReg::X2, XReg::X4);
        emit_darwin_read_syscall(&mut self.encoder);

        let failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Cs);
        self.encoder.emit_mov_x(XReg::X1, XReg::X0);
        emit_mov_i32_to_w0(&mut self.encoder, 0);
        let normalized_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(failure_branch, "read syscall failure target")?;
        self.emit_read_failure_payload_from_errno()?;

        self.patch_branch_placeholder_to_current(normalized_branch, "read syscall result target")?;
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        self.patch_branch_placeholder_to_current(success_branch, "read syscall success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_to_usize_location(XReg::X16, destination)
    }

    fn emit_open_read(
        &mut self,
        destination: I32Location,
        path: &UsizeValue,
        failure_mode: &FallibleFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "file open emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(path, XReg::X0)?;
        emit_mov_u64_to_x(&mut self.encoder, XReg::X1, 0);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, 0);
        emit_darwin_open_syscall(&mut self.encoder);

        let failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Cs);
        self.encoder.emit_mov_w(WReg::W1, WReg::W0);
        emit_mov_i32_to_w0(&mut self.encoder, 0);
        let normalized_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(failure_branch, "open syscall failure target")?;
        self.emit_open_failure_payload_from_errno()?;

        self.patch_branch_placeholder_to_current(normalized_branch, "open syscall result target")?;
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        self.patch_branch_placeholder_to_current(success_branch, "open syscall success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_i32_location(WReg::W16, destination)
    }

    fn emit_close_fd(
        &mut self,
        fd: &I32Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fd close emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_i32_value_to_w(fd, WReg::W0)?;
        emit_darwin_close_syscall(&mut self.encoder);
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    fn emit_set_usize_from_borrow(
        &mut self,
        destination: UsizeLocation,
        source: BorrowSource,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "borrow-to-pointer emission requires a stack frame",
            )]);
        };
        self.emit_borrow_source_address_to_x(source, XReg::X16, frame)?;
        self.emit_x_to_usize_location(XReg::X16, destination)
    }

    fn emit_static_data_address(&mut self, register: XReg, bytes: &[u8]) {
        let data_offset = self.read_only_data.len();
        self.read_only_data.extend_from_slice(bytes);

        let instruction_offset = self.encoder.position();
        self.encoder.emit_adr_x(register, 0);
        self.data_address_patches.push(DataAddressPatch {
            instruction_offset,
            register,
            data_offset,
        });
    }

    fn finish(mut self) -> Result<MachineCode, Vec<Diagnostic>> {
        self.patch_function_calls()?;

        let read_only_data_base_offset = align_usize(self.encoder.position(), 8);

        for patch in &self.data_address_patches {
            let data_offset = read_only_data_base_offset + patch.data_offset;
            let byte_offset = data_offset as i64 - patch.instruction_offset as i64;
            if !(ADR_MIN_BYTE_OFFSET..=ADR_MAX_BYTE_OFFSET).contains(&byte_offset) {
                return Err(vec![Diagnostic::error(
                    "E9001",
                    "static data is too far from generated code for ARM64 `adr`",
                )]);
            }

            self.encoder
                .patch_adr_x(patch.instruction_offset, patch.register, byte_offset as i32);
        }

        Ok(MachineCode {
            text: self.encoder.finish(),
            read_only_data: self.read_only_data,
        })
    }

    fn patch_function_calls(&mut self) -> Result<(), Vec<Diagnostic>> {
        for patch in &self.call_patches {
            let byte_offset = self.function_call_byte_offset(patch, "bl")?;
            self.encoder
                .patch_bl(patch.instruction_offset, byte_offset as i32);
        }

        for patch in &self.tail_call_patches {
            let byte_offset = self.function_call_byte_offset(patch, "b")?;
            self.encoder
                .patch_b(patch.instruction_offset, byte_offset as i32);
        }

        Ok(())
    }

    fn function_call_byte_offset(
        &self,
        patch: &FunctionCallPatch,
        instruction: &str,
    ) -> Result<i64, Vec<Diagnostic>> {
        let Some(target_offset) = self.function_offsets.get(&patch.function) else {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "codegen could not resolve function `{}`",
                    patch.function.description()
                ),
            )]);
        };

        let byte_offset = *target_offset as i64 - patch.instruction_offset as i64;
        if !(BRANCH_MIN_BYTE_OFFSET..=BRANCH_MAX_BYTE_OFFSET).contains(&byte_offset) {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "function `{}` is too far from call site for ARM64 `{instruction}`",
                    patch.function.description()
                ),
            )]);
        }

        Ok(byte_offset)
    }
}

#[cfg(test)]
mod tests;
