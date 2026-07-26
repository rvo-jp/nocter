use crate::abi::ReturnPassing;
use crate::backend::frame::{FrameLayout, FunctionFrame, plan_function_frame};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::{
    CallTarget, DirectAggregateArgument, FallibleFailureMode, Function, I32Location, I32Value,
    Instruction, IrModule, ScalarArgument, SliceValue, StrLocation, StrValue, Type, UsizeLocation,
    UsizeValue,
};
use crate::target::arm64::{BranchCondition, Encoder, MoveWideShift, WReg, XReg};
use std::collections::HashMap;

mod calls;
mod control_flow;
mod locations;
mod values;

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
            Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_i32(*destination, *offset, value, frame)?;
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
            Instruction::StoreAggregateBool {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_bool(*destination, *offset, value, frame)?;
            }
            Instruction::LoadAggregateUsize {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_usize(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateI32 {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_i32(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateU8 {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_u8(*destination, *source, *offset, frame)?;
            }
            Instruction::LoadAggregateBool {
                destination,
                source,
                offset,
            } => {
                self.emit_load_aggregate_bool(*destination, *source, *offset, frame)?;
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

fn validate_supported_fallible_success_payload_abi(
    success_type: &Type,
) -> Result<(), Vec<Diagnostic>> {
    match success_type.success_return_passing() {
        Some(ReturnPassing::Void | ReturnPassing::IndirectPointer) => Ok(()),
        Some(ReturnPassing::Direct { words }) => {
            if words <= FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT {
                return Ok(());
            }
            Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "fallible success payload uses {words} direct ABI words, but codegen supports at most {FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT}"
                ),
            )])
        }
        Some(ReturnPassing::Never) | None => Err(vec![Diagnostic::error(
            "E9002",
            "invalid fallible success payload ABI for codegen",
        )]),
    }
}

#[derive(Debug, Clone, Copy)]
enum ExpectedCallReturnShape {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice,
    Void,
    IndirectAggregate,
    DirectAggregate { layout: crate::abi::ValueLayout },
}

impl ExpectedCallReturnShape {
    fn passing(self) -> Option<ReturnPassing> {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => {
                Some(ReturnPassing::Direct { words: 1 })
            }
            Self::Str | Self::Slice => Some(ReturnPassing::Direct { words: 2 }),
            Self::Void => Some(ReturnPassing::Void),
            Self::IndirectAggregate => Some(ReturnPassing::IndirectPointer),
            Self::DirectAggregate { layout } => direct_aggregate_layout_passing(layout),
        }
    }

    fn matches_success_type(self, ty: &Type) -> bool {
        match (self, ty) {
            (Self::I32, Type::I32)
            | (Self::U8, Type::U8)
            | (Self::Usize, Type::Usize)
            | (Self::Bool, Type::Bool)
            | (Self::Str, Type::Str)
            | (Self::Slice, Type::Slice { .. })
            | (Self::Void, Type::Void)
            | (Self::IndirectAggregate, Type::Aggregate { .. }) => true,
            (Self::DirectAggregate { layout }, Type::DirectAggregate { layout: actual, .. }) => {
                layout == *actual
            }
            _ => false,
        }
    }

    fn description(self) -> String {
        match self {
            Self::I32 => "i32".to_string(),
            Self::U8 => "u8".to_string(),
            Self::Usize => "usize".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Str => "&str".to_string(),
            Self::Slice => "slice".to_string(),
            Self::Void => "void".to_string(),
            Self::IndirectAggregate => "indirect aggregate".to_string(),
            Self::DirectAggregate { layout } => format!(
                "direct aggregate {} ({})",
                layout_description(layout),
                return_passing_description(self.passing())
            ),
        }
    }
}

fn validate_module_call_return_shapes(module: &IrModule) -> Result<(), Vec<Diagnostic>> {
    let return_types = module
        .functions
        .iter()
        .map(|function| {
            (
                FunctionSymbol::from_function(function),
                &function.return_type,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();

    for function in &module.functions {
        validate_function_return_type_shape(function, &mut diagnostics);
        validate_instruction_list_call_return_shapes(
            &function.instructions,
            &function.return_type,
            &return_types,
            &mut diagnostics,
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_function_return_type_shape(function: &Function, diagnostics: &mut Vec<Diagnostic>) {
    validate_return_type_shape(
        &function.return_type,
        &format!("function `{}` return type", function.name),
        diagnostics,
    );
}

fn validate_return_type_shape(ty: &Type, subject: &str, diagnostics: &mut Vec<Diagnostic>) {
    match ty {
        Type::DirectAggregate { layout, words } => {
            validate_direct_aggregate_type_shape(*layout, *words, subject, diagnostics);
        }
        Type::Fallible(success) => {
            let subject = format!("{subject} fallible success type");
            validate_return_type_shape(success, &subject, diagnostics);
        }
        _ => {}
    }
}

fn validate_direct_aggregate_type_shape(
    layout: crate::abi::ValueLayout,
    words: usize,
    subject: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected_words) = direct_aggregate_layout_word_count(layout) else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!("codegen {subject} layout exceeds host word count range"),
        ));
        return;
    };
    if expected_words > DIRECT_AGGREGATE_REGISTER_WORD_COUNT {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen {subject} requires {expected_words} direct ABI words, but direct aggregate codegen supports at most {DIRECT_AGGREGATE_REGISTER_WORD_COUNT}"
            ),
        ));
        return;
    }
    if words == expected_words {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen {subject} uses {words} ABI words, but layout {} requires {expected_words}",
            layout_description(layout),
        ),
    ));
}

fn validate_instruction_list_call_return_shapes(
    instructions: &[Instruction],
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for instruction in instructions {
        if let Some(arguments) = instruction_call_arguments(instruction) {
            validate_call_argument_shapes(arguments, diagnostics);
        }

        match instruction {
            Instruction::CallI32 { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::I32,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleI32 {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::I32,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallU8 { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::U8,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleU8 {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::U8,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallUsize { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Usize,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleUsize {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Usize,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::ReadSlice { failure_mode, .. } => {
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::OpenRead { failure_mode, .. } => {
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallBool { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Bool,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleBool {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Bool,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallStr { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Str,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleStr {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Str,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallSlice { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Slice,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleSlice {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Slice,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallAggregate { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::IndirectAggregate,
                return_types,
                diagnostics,
            ),
            Instruction::CallDirectAggregate { target, layout, .. } => {
                validate_normal_call_return_shape(
                    target,
                    ExpectedCallReturnShape::DirectAggregate { layout: *layout },
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallFallibleDirectAggregate {
                target,
                layout,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::DirectAggregate { layout: *layout },
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallFallibleAggregate {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::IndirectAggregate,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallVoid { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Void,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleVoid {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Void,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::TailCall { target, .. } => {
                validate_tail_call_return_shape(
                    target,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                validate_instruction_list_call_return_shapes(
                    then_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
                validate_instruction_list_call_return_shapes(
                    else_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                validate_instruction_list_call_return_shapes(
                    condition_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
                validate_instruction_list_call_return_shapes(
                    body_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CheckFailure { failure_mode } => validate_failure_mode_call_return_shapes(
                failure_mode,
                current_return_type,
                return_types,
                diagnostics,
            ),
            _ => {}
        }
    }
}

fn instruction_call_arguments(instruction: &Instruction) -> Option<&[ScalarArgument]> {
    match instruction {
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallFallibleI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallFallibleU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallFallibleUsize { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallFallibleBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallFallibleStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallFallibleSlice { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallFallibleDirectAggregate { arguments, .. }
        | Instruction::CallFallibleAggregate { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::CallFallibleVoid { arguments, .. }
        | Instruction::TailCall { arguments, .. } => Some(arguments),
        _ => None,
    }
}

fn validate_call_argument_shapes(arguments: &[ScalarArgument], diagnostics: &mut Vec<Diagnostic>) {
    for argument in arguments {
        if let ScalarArgument::AggregateDirect(argument) = argument {
            validate_direct_aggregate_argument_shape(argument, diagnostics);
        }
    }
}

fn validate_direct_aggregate_argument_shape(
    argument: &DirectAggregateArgument,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected_words) = direct_aggregate_layout_word_count(argument.layout) else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            "codegen direct aggregate argument layout exceeds host word count range",
        ));
        return;
    };
    if argument.words == expected_words {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen direct aggregate argument uses {} ABI words, but layout {} requires {expected_words}",
            argument.words,
            layout_description(argument.layout),
        ),
    ));
}

fn validate_failure_mode_call_return_shapes(
    failure_mode: &FallibleFailureMode,
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            validate_instruction_list_call_return_shapes(
                instructions,
                current_return_type,
                return_types,
                diagnostics,
            );
        }
    }
}

fn validate_normal_call_return_shape(
    target: &CallTarget,
    expected: ExpectedCallReturnShape,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    if matches!(return_type, Type::Fallible(_)) {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen normal call to function `{}` targets a fallible return",
                function.description()
            ),
        ));
        return;
    }
    validate_success_return_shape(&function, return_type, expected, "normal call", diagnostics);
}

fn validate_fallible_call_return_shape(
    target: &CallTarget,
    expected: ExpectedCallReturnShape,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    let Type::Fallible(success_type) = return_type else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen fallible call to function `{}` targets a non-fallible return",
                function.description()
            ),
        ));
        return;
    };
    validate_success_return_shape(
        &function,
        success_type,
        expected,
        "fallible call success",
        diagnostics,
    );
}

fn validate_tail_call_return_shape(
    target: &CallTarget,
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    if return_type.success_return_passing() == Some(ReturnPassing::Never) {
        return;
    }
    if *return_type == current_return_type {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen tail call return mismatch for function `{}`: expected {}, got {}",
            function.description(),
            type_return_description(current_return_type),
            type_return_description(return_type),
        ),
    ));
}

fn validate_success_return_shape(
    function: &FunctionSymbol,
    success_type: &Type,
    expected: ExpectedCallReturnShape,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected_passing = expected.passing();
    let actual_passing = success_type.success_return_passing();
    if expected.matches_success_type(success_type) && expected_passing == actual_passing {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen {context} return mismatch for function `{}`: expected {} ({}), got {}",
            function.description(),
            expected.description(),
            return_passing_description(expected_passing),
            type_return_description(success_type),
        ),
    ));
}

fn direct_aggregate_layout_passing(layout: crate::abi::ValueLayout) -> Option<ReturnPassing> {
    direct_aggregate_layout_word_count(layout).map(|words| ReturnPassing::Direct { words })
}

fn direct_aggregate_layout_word_count(layout: crate::abi::ValueLayout) -> Option<usize> {
    usize::try_from(layout.size.div_ceil(crate::abi::ABI_WORD_SIZE)).ok()
}

fn registers_overlap(destinations: &[XReg], sources: &[Option<XReg>; 2]) -> bool {
    destinations
        .iter()
        .any(|destination| sources.iter().any(|source| source == &Some(*destination)))
}

fn failure_payload_temporary_pair(
    protected_sources: &[Option<XReg>; 2],
    protected_destinations: &[XReg; 2],
) -> Result<(XReg, XReg), Vec<Diagnostic>> {
    let candidates = [
        XReg::X5,
        XReg::X6,
        XReg::X7,
        XReg::X9,
        XReg::X10,
        XReg::X11,
        XReg::X12,
        XReg::X13,
        XReg::X14,
        XReg::X15,
    ];
    let selected = candidates
        .into_iter()
        .filter(|register| {
            !protected_destinations.contains(register)
                && !protected_sources
                    .iter()
                    .any(|source| source == &Some(*register))
        })
        .take(2)
        .collect::<Vec<_>>();

    let [ptr, len] = selected.as_slice() else {
        return Err(vec![Diagnostic::error(
            "E9005",
            "codegen cannot allocate temporary registers for fallible failure payload",
        )]);
    };
    Ok((*ptr, *len))
}

fn module_uses_process_arguments(module: &IrModule) -> bool {
    module
        .functions
        .iter()
        .any(|function| instructions_use_process_arguments(&function.instructions))
}

fn instructions_use_process_arguments(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_uses_process_arguments)
}

fn instruction_uses_process_arguments(instruction: &Instruction) -> bool {
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
            path, failure_mode, ..
        } => {
            usize_value_uses_process_arguments(path)
                || failure_mode_uses_process_arguments(failure_mode)
        }
        Instruction::CloseFd { fd } | Instruction::ProcessExit { code: fd } => {
            i32_value_uses_process_arguments(fd)
        }
        Instruction::SetI32 { value, .. } => i32_value_uses_process_arguments(value),
        Instruction::SetU8 { value, .. } => u8_value_uses_process_arguments(value),
        Instruction::SetUsize { value, .. } => usize_value_uses_process_arguments(value),
        Instruction::SetBool { value, .. } => bool_value_uses_process_arguments(value),
        Instruction::SetStr { value, .. } => str_value_uses_process_arguments(value),
        Instruction::SetStrRawParts { pointer, len, .. }
        | Instruction::SetSliceRawParts { pointer, len, .. } => {
            usize_value_uses_process_arguments(pointer) || usize_value_uses_process_arguments(len)
        }
        Instruction::SetSlice { value, .. } => slice_value_uses_process_arguments(value),
        Instruction::ReserveAggregateSlot { .. }
        | Instruction::CopyAggregate { .. }
        | Instruction::CopyAggregateRange { .. }
        | Instruction::CopySliceElementToAggregate { .. }
        | Instruction::LoadAggregateUsize { .. }
        | Instruction::LoadAggregateI32 { .. }
        | Instruction::LoadAggregateU8 { .. }
        | Instruction::LoadAggregateBool { .. } => false,
        Instruction::StoreAggregateUsize { value, .. } => usize_value_uses_process_arguments(value),
        Instruction::StoreAggregateI32 { value, .. } => i32_value_uses_process_arguments(value),
        Instruction::StoreAggregateU16 { .. } | Instruction::StoreAggregateU32 { .. } => false,
        Instruction::StoreAggregateU8 { value, .. } => u8_value_uses_process_arguments(value),
        Instruction::StoreAggregateBool { value, .. } => bool_value_uses_process_arguments(value),
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
        Instruction::StoreBoolToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || bool_value_uses_process_arguments(value)
        }
        Instruction::StoreStrToSliceIndex { index, value, .. } => {
            usize_value_uses_process_arguments(index) || str_value_uses_process_arguments(value)
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
            scalar_arguments_use_process_arguments(arguments)
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
            scalar_arguments_use_process_arguments(arguments)
                || failure_mode_uses_process_arguments(failure_mode)
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
        | Instruction::ReturnFallibleSuccess
        | Instruction::ReturnOptionalNone
        | Instruction::Return => false,
    }
}

fn failure_mode_uses_process_arguments(failure_mode: &FallibleFailureMode) -> bool {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => false,
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            instructions_use_process_arguments(instructions)
        }
    }
}

fn scalar_arguments_use_process_arguments(arguments: &[ScalarArgument]) -> bool {
    arguments.iter().any(scalar_argument_uses_process_arguments)
}

fn scalar_argument_uses_process_arguments(argument: &ScalarArgument) -> bool {
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

fn i32_value_uses_process_arguments(value: &I32Value) -> bool {
    match value {
        I32Value::Const(_) | I32Value::Location(_) => false,
        I32Value::U8ZeroExtend(value) => u8_value_uses_process_arguments(value),
        I32Value::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

fn u8_value_uses_process_arguments(value: &crate::ir::U8Value) -> bool {
    match value {
        crate::ir::U8Value::Const(_) | crate::ir::U8Value::Location(_) => false,
        crate::ir::U8Value::StrIndex { index, .. }
        | crate::ir::U8Value::StaticStrIndex { index, .. }
        | crate::ir::U8Value::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

fn usize_value_uses_process_arguments(value: &UsizeValue) -> bool {
    match value {
        UsizeValue::ProcessArgCount => true,
        UsizeValue::Const(_) | UsizeValue::Location(_) => false,
        UsizeValue::U8ZeroExtend(value) => u8_value_uses_process_arguments(value),
        UsizeValue::StrLen(_) | UsizeValue::SliceLen(_) => false,
        UsizeValue::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

fn bool_value_uses_process_arguments(value: &crate::ir::BoolValue) -> bool {
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
        crate::ir::BoolValue::StrComparison { left, right, .. } => {
            str_value_uses_process_arguments(left) || str_value_uses_process_arguments(right)
        }
    }
}

fn str_value_uses_process_arguments(value: &StrValue) -> bool {
    match value {
        StrValue::ProcessArg { .. } => true,
        StrValue::StaticBytes(_) | StrValue::Location(_) => false,
        StrValue::SliceIndex { index, .. } => usize_value_uses_process_arguments(index),
    }
}

fn slice_value_uses_process_arguments(value: &SliceValue) -> bool {
    match value {
        SliceValue::Location(_) => false,
        SliceValue::StrBytes(text) => str_value_uses_process_arguments(text),
    }
}

fn checked_pair_len_index(first_index: usize, subject: &str) -> Result<usize, Vec<Diagnostic>> {
    first_index.checked_add(1).ok_or_else(|| {
        vec![Diagnostic::error(
            "E9005",
            format!("{subject} length word index overflows"),
        )]
    })
}

fn return_passing_description(passing: Option<ReturnPassing>) -> &'static str {
    passing.map_or("unsupported return ABI", ReturnPassing::description)
}

fn type_return_description(ty: &Type) -> String {
    let shape = match ty {
        Type::I32 => "i32".to_string(),
        Type::U8 => "u8".to_string(),
        Type::Usize => "usize".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Str => "&str".to_string(),
        Type::Slice { .. } => "slice".to_string(),
        Type::Aggregate { layout } => {
            format!("indirect aggregate {}", layout_description(*layout))
        }
        Type::DirectAggregate { layout, .. } => {
            format!("direct aggregate {}", layout_description(*layout))
        }
        Type::Borrow { .. } => "borrow".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::Fallible(success) => format!("fallible {}", type_return_description(success)),
    };
    format!(
        "{shape} ({})",
        return_passing_description(ty.success_return_passing())
    )
}

fn layout_description(layout: crate::abi::ValueLayout) -> String {
    format!("{} bytes align {}", layout.size, layout.align)
}

fn unresolved_call_target_diagnostic(function: &FunctionSymbol) -> Diagnostic {
    Diagnostic::error(
        "E9002",
        format!(
            "codegen could not resolve function `{}`",
            function.description()
        ),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionSymbol {
    SameFile(String),
    Imported {
        source: crate::source::SourceId,
        name: String,
    },
}

impl FunctionSymbol {
    fn same_file(name: impl Into<String>) -> Self {
        Self::SameFile(name.into())
    }

    fn from_function(function: &Function) -> Self {
        Self::from_call_target(&function.target)
    }

    fn from_call_target(target: &CallTarget) -> Self {
        match target {
            CallTarget::SameFile(name) => Self::same_file(name),
            CallTarget::Imported { source, name } => Self::Imported {
                source: *source,
                name: name.clone(),
            },
        }
    }

    fn description(&self) -> String {
        match self {
            Self::SameFile(name) => name.clone(),
            Self::Imported { source, name } => {
                format!("{} from source {}", name, source.raw())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DataAddressPatch {
    instruction_offset: usize,
    register: XReg,
    data_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionCallPatch {
    instruction_offset: usize,
    function: FunctionSymbol,
}

fn emit_mov_i32_to_w0(encoder: &mut Encoder, value: i32) {
    emit_mov_i32_to_w(encoder, WReg::W0, value);
}

fn emit_mov_i32_to_w(encoder: &mut Encoder, register: WReg, value: i32) {
    emit_mov_u32_to_w(encoder, register, value as u32);
}

fn emit_mov_u32_to_w(encoder: &mut Encoder, register: WReg, value: u32) {
    encoder.emit_movz_w(register, value as u16, MoveWideShift::Lsl0);

    let high = value >> 16;
    if high != 0 {
        encoder.emit_movk_w(register, high as u16, MoveWideShift::Lsl16);
    }
}

fn emit_mov_u64_to_x(encoder: &mut Encoder, register: XReg, value: u64) {
    encoder.emit_movz_x(register, value as u16, MoveWideShift::Lsl0);

    for (shift, amount) in [
        (MoveWideShift::Lsl16, 16),
        (MoveWideShift::Lsl32, 32),
        (MoveWideShift::Lsl48, 48),
    ] {
        let chunk = (value >> amount) as u16;
        if chunk != 0 {
            encoder.emit_movk_x(register, chunk, shift);
        }
    }
}

fn emit_darwin_exit_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_EXIT_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn emit_darwin_write_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_WRITE_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn emit_darwin_read_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_READ_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn emit_darwin_open_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_OPEN_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn emit_darwin_close_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_CLOSE_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

const STDERR_FILENO: u64 = 2;
const FALLIBLE_REPORT_FRAME_SIZE: u32 = 32;
const WRITE_LOOP_FRAME_SIZE: u32 = 32;
const WRITE_LOOP_FD_OFFSET: u32 = 0;
const WRITE_LOOP_POINTER_OFFSET: u32 = 8;
const WRITE_LOOP_REMAINING_OFFSET: u32 = 16;
const WRITE_UNEXPECTED_RESULT_ERRNO: u64 = 0xffff;
const FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT: usize = 2;
const DIRECT_AGGREGATE_REGISTER_WORD_COUNT: usize = 2;
const WRITE_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.write_failed",
    message: b"write failed",
};
const READ_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.read_failed",
    message: b"read failed",
};
const OPEN_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.open_failed",
    message: b"open failed",
};
const IO_INTERRUPTED_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.interrupted",
    message: b"operation interrupted",
};
const IO_WOULD_BLOCK_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.would_block",
    message: b"operation would block",
};
const IO_NOT_FOUND_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.not_found",
    message: b"file not found",
};
const IO_PERMISSION_DENIED_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.permission_denied",
    message: b"permission denied",
};
const IO_INVALID_INPUT_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.invalid_input",
    message: b"invalid I/O input",
};
const IO_BROKEN_PIPE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.broken_pipe",
    message: b"broken pipe",
};
const OPEN_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_ENOENT,
        payload: IO_NOT_FOUND_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_ENOTDIR,
        payload: IO_NOT_FOUND_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EPERM,
        payload: IO_PERMISSION_DENIED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EACCES,
        payload: IO_PERMISSION_DENIED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
];
const READ_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_EINTR,
        payload: IO_INTERRUPTED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EAGAIN,
        payload: IO_WOULD_BLOCK_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EBADF,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EISDIR,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
];
const WRITE_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_EINTR,
        payload: IO_INTERRUPTED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EAGAIN,
        payload: IO_WOULD_BLOCK_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EBADF,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EPIPE,
        payload: IO_BROKEN_PIPE_PAYLOAD,
    },
];
const ADR_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i64 = (1 << 20) - 1;
const BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 27);
const BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 27) - 4;
const DARWIN_READ_SYSCALL: u32 = 0x0200_0003;
const DARWIN_OPEN_SYSCALL: u32 = 0x0200_0005;
const DARWIN_WRITE_SYSCALL: u32 = 0x0200_0004;
const DARWIN_CLOSE_SYSCALL: u32 = 0x0200_0006;
const DARWIN_EXIT_SYSCALL: u32 = 0x0200_0001;
const DARWIN_SYSCALL_TRAP: u16 = 0x80;
const DARWIN_EPERM: i32 = 1;
const DARWIN_ENOENT: i32 = 2;
const DARWIN_EINTR: i32 = 4;
const DARWIN_EBADF: i32 = 9;
const DARWIN_EACCES: i32 = 13;
const DARWIN_EFAULT: i32 = 14;
const DARWIN_ENOTDIR: i32 = 20;
const DARWIN_EISDIR: i32 = 21;
const DARWIN_EINVAL: i32 = 22;
const DARWIN_EPIPE: i32 = 32;
const DARWIN_EAGAIN: i32 = 35;
const I32_BIT_WIDTH: i32 = 32;
const USIZE_BIT_WIDTH: u64 = 64;

#[cfg(test)]
mod tests {
    use super::control_flow::branch_condition_for_true_comparison;
    use super::*;
    use crate::abi::ValueLayout;
    use crate::ir::{
        AggregateArgumentSource, AggregateLocation, BoolLocation, BoolValue, BorrowArgument,
        BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, Function,
        I32ComparisonOperator, I32Location, I32Value, ScalarArgument, SliceElementAddressKind,
        SliceElementIndex, SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location,
        U8Value, UsizeLocation, UsizeValue,
    };
    use crate::source::SourceId;
    use crate::target::arm64::BranchCondition;

    #[test]
    fn maps_imported_call_target_to_imported_function_symbol() {
        let source = SourceId::new(9);
        let symbol = FunctionSymbol::from_call_target(&CallTarget::imported(source, "answer"));

        assert_eq!(
            symbol,
            FunctionSymbol::Imported {
                source,
                name: "answer".to_string(),
            }
        );
        assert_eq!(symbol.description(), "answer from source 9");
    }

    #[test]
    fn maps_function_definition_to_same_file_function_symbol() {
        let function = Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::Return],
        };

        assert_eq!(
            FunctionSymbol::from_function(&function),
            FunctionSymbol::SameFile("answer".to_string())
        );
    }

    #[test]
    fn maps_imported_function_definition_to_imported_function_symbol() {
        let source = SourceId::new(11);
        let function = Function {
            name: "answer".to_string(),
            target: CallTarget::imported(source, "answer"),
            return_type: Type::I32,
            instructions: vec![Instruction::Return],
        };

        assert_eq!(
            FunctionSymbol::from_function(&function),
            FunctionSymbol::Imported {
                source,
                name: "answer".to_string(),
            }
        );
    }

    #[test]
    fn generates_exit_zero_for_return_i32_zero() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_with_high_halfword() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0x1234_5678), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x00, 0xcf, 0x8a, 0x52, // movz w0, #0x5678
                0x80, 0x46, 0xa2, 0x72, // movk w0, #0x1234, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_return_i32_negative_one() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(-1), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xe0, 0xff, 0x9f, 0x52, // movz w0, #0xffff
                0xe0, 0xff, 0xbf, 0x72, // movk w0, #0xffff, lsl #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_fallible_success_return_i32() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnFallibleSuccess],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(code.read_only_data, b": \n");
        assert_eq!(
            code.text[code.text.len() - 16..],
            vec![
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xe1, 0x03, 0x00, 0x2a, // mov w1, w0
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_exit_code_for_fallible_success_return_usize() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::Usize)),
            instructions: vec![set_return_usize(7), Instruction::ReturnFallibleSuccess],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X1, XReg::X0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X0, XReg::X1)
        ));
    }

    #[test]
    fn rejects_direct_aggregate_return_wider_than_two_words_before_codegen() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Fallible(Box::new(Type::DirectAggregate {
                    layout: ValueLayout::new(24, 8),
                    words: 3,
                })),
                instructions: vec![Instruction::ReturnFallibleSuccess],
            },
        ]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert_eq!(diagnostics[0].code, "E9002");
        assert!(
            diagnostics[0]
                .message
                .contains("requires 3 direct ABI words")
        );
    }

    #[test]
    fn rejects_normal_call_to_fallible_callee_before_codegen() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Return,
                        target: CallTarget::same_file("answer"),
                        arguments: vec![],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::Fallible(Box::new(Type::I32)),
                instructions: vec![Instruction::ReturnFallibleSuccess],
            },
        ]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert_eq!(diagnostics[0].code, "E9002");
        assert!(
            diagnostics[0]
                .message
                .contains("normal call to function `answer` targets a fallible return")
        );
    }

    #[test]
    fn rejects_call_return_shape_mismatch_before_codegen() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Return,
                        target: CallTarget::same_file("title"),
                        arguments: vec![],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "title".to_string(),
                target: crate::ir::CallTarget::same_file("title".to_string()),
                return_type: Type::Str,
                instructions: vec![Instruction::Return],
            },
        ]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert_eq!(diagnostics[0].code, "E9002");
        assert!(diagnostics[0].message.contains("expected i32"));
        assert!(diagnostics[0].message.contains("got &str"));
    }

    #[test]
    fn rejects_direct_aggregate_argument_word_count_mismatch_before_codegen() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(16, 8),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("consume"),
                        arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                            source: AggregateArgumentSource::Slot(0),
                            layout: ValueLayout::new(16, 8),
                            words: 1,
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert_eq!(diagnostics[0].code, "E9002");
        assert!(
            diagnostics[0]
                .message
                .contains("direct aggregate argument uses 1 ABI words")
        );
        assert!(diagnostics[0].message.contains("requires 2"));
    }

    #[test]
    fn emits_framed_function_prologue_and_return_epilogue() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(7), Instruction::Return],
        };
        let frame = FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 0).unwrap());
        let mut emitter = EntryEmitter::new();

        emitter.emit_function_with_frame(&function, &frame).unwrap();

        assert_eq!(
            emitter.encoder.finish(),
            vec![
                0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
                0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
                0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn emits_framed_tail_call_epilogue_before_branch() {
        let main = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![tail_call("answer", vec![])],
        };
        let answer = Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(7), Instruction::Return],
        };
        let frame = FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 0).unwrap());
        let mut emitter = EntryEmitter::new();

        emitter.emit_function_with_frame(&main, &frame).unwrap();
        emitter.emit_function(&answer).unwrap();
        let code = emitter.finish().unwrap();

        assert_eq!(
            code.text,
            vec![
                0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
                0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
                0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
                0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
                0x01, 0x00, 0x00, 0x14, // b answer
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn tail_call_rejects_borrow_arguments() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(7),
                    },
                    Instruction::TailCall {
                        target: CallTarget::same_file("consume"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::I32(I32Location::Local(0)),
                        })],
                    },
                ],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
        ]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert_eq!(diagnostics[0].code, "E9003");
        assert!(diagnostics[0].message.contains("borrow arguments"));
    }

    #[test]
    fn emits_trap_instruction() {
        let function = Function {
            name: "abort".to_string(),
            target: crate::ir::CallTarget::same_file("abort".to_string()),
            return_type: Type::Never,
            instructions: vec![Instruction::Trap],
        };
        let mut emitter = EntryEmitter::new();

        emitter.emit_function(&function).unwrap();

        assert_eq!(
            emitter.encoder.finish(),
            vec![
                0x00, 0x00, 0x20, 0xd4, // brk #0
            ]
        );
    }

    #[test]
    fn emits_static_str_return_data() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "title".to_string(),
                target: crate::ir::CallTarget::same_file("title".to_string()),
                return_type: Type::Str,
                instructions: vec![
                    Instruction::SetStr {
                        destination: StrLocation::Return,
                        value: StrValue::StaticBytes(b"Nocter".to_vec()),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(code.read_only_data, b"Nocter");
    }

    #[test]
    fn generates_framed_i32_normal_call_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Return, "answer", vec![]),
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
                0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
                0x04, 0x00, 0x00, 0x94, // bl answer
                0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
                0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_framed_void_normal_call_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_void("effect", vec![]),
                    set_return_i32(7),
                    Instruction::Return,
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
                0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
                0x05, 0x00, 0x00, 0x94, // bl effect
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
                0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_slice_normal_call_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "wrapper".to_string(),
                target: crate::ir::CallTarget::same_file("wrapper".to_string()),
                return_type: readonly_u8_slice_type(),
                instructions: vec![
                    Instruction::CallSlice {
                        destination: SliceLocation::Return,
                        target: CallTarget::same_file("identity"),
                        arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                            SliceLocation::Parameter(0),
                        ))],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: readonly_u8_slice_type(),
                instructions: vec![
                    Instruction::SetSlice {
                        destination: SliceLocation::Return,
                        value: SliceValue::Location(SliceLocation::Parameter(0)),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(!code.text.is_empty());
    }

    #[test]
    fn generates_str_len_value_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "size".to_string(),
                target: crate::ir::CallTarget::same_file("size".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: UsizeValue::StrLen(StrLocation::Parameter(0)),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
    }

    #[test]
    fn generates_slice_len_value_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "size".to_string(),
                target: crate::ir::CallTarget::same_file("size".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: UsizeValue::SliceLen(SliceLocation::Parameter(0)),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
    }

    #[test]
    fn generates_slice_index_byte_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(1),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
    }

    #[test]
    fn generates_slice_index_usize_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: UsizeValue::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: Box::new(UsizeValue::Const(1)),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 3),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_adds_x(XReg::X17, XReg::X17, XReg::X16),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_imm(XReg::X0, XReg::X17, 0),
        ));
    }

    #[test]
    fn generates_slice_index_i32_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: I32Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(1),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_w_reg(WReg::W0, XReg::X17, XReg::X16),
        ));
    }

    #[test]
    fn generates_slice_index_bool_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(1),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_ldrb_w_reg(WReg::W0, XReg::X17, XReg::X16),
        ));
    }

    #[test]
    fn generates_stack_passed_slice_index_byte_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: U8Value::SliceIndex {
                            source: SliceLocation::Parameter(8),
                            index: UsizeValue::Const(1),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0xf1, 0x07, 0x40, 0xf9])); // ldr x17, [sp, #8]
        assert!(contains_instruction(&code.text, [0xf1, 0x03, 0x40, 0xf9])); // ldr x17, [sp, #0]
        assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
    }

    #[test]
    fn generates_static_str_index_byte_load_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: U8Value::StaticStrIndex {
                            bytes: b"Nocter".to_vec(),
                            index: UsizeValue::Const(3),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
        assert_eq!(code.read_only_data, b"Nocter");
    }

    #[test]
    fn generates_u8_to_i32_zero_extend_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "first".to_string(),
                target: crate::ir::CallTarget::same_file("first".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(1),
                        })),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
    }

    #[test]
    fn generates_u8_comparison_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "check".to_string(),
                target: crate::ir::CallTarget::same_file("check".to_string()),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                                source: SliceLocation::Parameter(0),
                                index: UsizeValue::Const(0),
                            })),
                            right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(0x7F))),
                        },
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x30, 0x6a, 0x70, 0x38])); // ldrb w16, [x17, x16]
        assert!(contains_instruction(&code.text, [0x20, 0x00, 0x80, 0x52])); // mov w0, #1
    }

    #[test]
    fn normal_i32_call_spills_and_reloads_scalar_locals() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(40),
                    },
                    call_i32(I32Location::Local(1), "add_two", vec![i32_local(0)]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "add_two".to_string(),
                target: crate::ir::CallTarget::same_file("add_two".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_const(2),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xff, 0x83, 0x00, 0xd1, // sub sp, sp, #32
                0xfe, 0x0f, 0x00, 0xf9, // str x30, [sp, #24]
                0x09, 0x05, 0x80, 0x52, // movz w9, #40
                0xe9, 0x03, 0x00, 0xf9, // str x9, [sp, #0]
                0xea, 0x07, 0x00, 0xf9, // str x10, [sp, #8]
                0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
                0xf0, 0x13, 0x00, 0xb9, // str w16, [sp, #16]
                0xe0, 0x13, 0x40, 0xb9, // ldr w0, [sp, #16]
                0x0c, 0x00, 0x00, 0x94, // bl add_two
                0xe9, 0x03, 0x40, 0xf9, // ldr x9, [sp, #0]
                0xea, 0x07, 0x40, 0xf9, // ldr x10, [sp, #8]
                0xea, 0x03, 0x00, 0x2a, // mov w10, w0
                0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
                0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
                0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
                0x47, 0x00, 0x00, 0x54, // b.vc +8
                0x00, 0x00, 0x20, 0xd4, // brk #0
                0xfe, 0x0f, 0x40, 0xf9, // ldr x30, [sp, #24]
                0xff, 0x83, 0x00, 0x91, // add sp, sp, #32
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0xf0, 0x03, 0x00, 0x2a, // mov w16, w0
                0x40, 0x00, 0x80, 0x52, // movz w0, #2
                0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
                0x47, 0x00, 0x00, 0x54, // b.vc +8
                0x00, 0x00, 0x20, 0xd4, // brk #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn normal_call_can_pass_scalar_parameter_borrow() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::same_file("caller"),
                    arguments: vec![ScalarArgument::I32(i32_const(7))],
                }],
            },
            Function {
                name: "caller".to_string(),
                target: crate::ir::CallTarget::same_file("caller".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Return,
                        target: CallTarget::same_file("choose"),
                        arguments: vec![
                            ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::I32(I32Location::Parameter(0)),
                            }),
                            ScalarArgument::I32(i32_const(42)),
                        ],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(!code.text.is_empty());
    }

    #[test]
    fn normal_call_can_pass_stack_scalar_parameter_borrow() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "caller".to_string(),
                target: crate::ir::CallTarget::same_file("caller".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Return,
                        target: CallTarget::same_file("choose"),
                        arguments: vec![
                            ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::I32(I32Location::Parameter(8)),
                            }),
                            ScalarArgument::I32(i32_const(42)),
                        ],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(!code.text.is_empty());
    }

    #[test]
    fn aggregate_call_passes_destination_slot_in_x8() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::CallAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 16,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
    }

    #[test]
    fn aggregate_return_call_restores_saved_x8_destination() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                instructions: vec![
                    Instruction::CallAggregate {
                        destination: AggregateLocation::Return,
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 16,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(!contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X8, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X8, 0)
        ));
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
    }

    #[test]
    fn aggregate_scalar_field_stores_use_field_width() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(32, 8),
                },
                instructions: vec![
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Return,
                        offset: 0,
                        value: U8Value::Const(7),
                    },
                    Instruction::StoreAggregateBool {
                        destination: AggregateLocation::Return,
                        offset: 1,
                        value: BoolValue::Const(true),
                    },
                    Instruction::StoreAggregateU16 {
                        destination: AggregateLocation::Return,
                        offset: 2,
                        value: 0xaabb,
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Return,
                        offset: 4,
                        value: I32Value::Const(42),
                    },
                    Instruction::StoreAggregateU32 {
                        destination: AggregateLocation::Return,
                        offset: 12,
                        value: 0xaabb_ccdd,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 16,
                        value: UsizeValue::Const(11),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W16, XReg::X8, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W16, XReg::X8, 1)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strh_w_imm(WReg::W16, XReg::X8, 2)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W16, XReg::X8, 4)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W16, XReg::X8, 12)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X16, XReg::X8, 16)
        ));
    }

    #[test]
    fn pointer_u8_store_uses_byte_store() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::StoreU8ToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(4),
                    value: U8Value::Const(0),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn pointer_i32_store_uses_word_store() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::StoreI32ToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(4),
                    value: I32Value::Const(42),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn pointer_usize_store_uses_word_store() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::StoreUsizeToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(8),
                    value: UsizeValue::Const(42),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X2, XReg::X0, 0)
        ));
    }

    #[test]
    fn pointer_bool_store_uses_byte_store() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::StoreBoolToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(1),
                    value: BoolValue::Const(true),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn pointer_str_store_uses_two_word_stores() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::StoreStrToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(16),
                    value: StrValue::StaticBytes(b"arg".to_vec()),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X2, XReg::X0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X3, XReg::X0, 8)
        ));
    }

    #[test]
    fn pointer_aggregate_copy_stores_slot_bytes() {
        let layout = ValueLayout { size: 4, align: 4 };
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregateToPointer {
                    pointer: UsizeValue::Const(4096),
                    offset: UsizeValue::Const(4),
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W16, XReg::X9, 0)
        ));
    }

    #[test]
    fn slice_u8_index_store_uses_byte_store() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "fill".to_string(),
                target: crate::ir::CallTarget::same_file("fill".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreU8ToSliceIndex {
                        destination: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(0),
                        value: U8Value::Const(7),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn slice_i32_index_store_uses_word_store() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "fill".to_string(),
                target: crate::ir::CallTarget::same_file("fill".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreI32ToSliceIndex {
                        destination: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                        value: I32Value::Const(42),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        let value_offset = instruction_offset(&code.text, &encoded_mov_u32_to_w(WReg::W2, 42))
            .expect("expected i32 store value materialization");
        let scale_offset =
            instruction_offset(&code.text, &encoded_lsl_x_imm(XReg::X16, XReg::X16, 2))
                .expect("expected i32 slice index scaling");
        assert!(
            value_offset < scale_offset,
            "slice index stores must materialize the assigned value before computing the destination address"
        );
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn slice_i32_index_borrow_argument_uses_element_address() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::SliceIndex {
                                source: SliceLocation::Parameter(0),
                                index: SliceElementIndex::Const(1),
                                element: SliceElementAddressKind::I32,
                            },
                        })],
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "touch".to_string(),
                target: crate::ir::CallTarget::same_file("touch".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_adds_x(XReg::X16, XReg::X8, XReg::X16),
        ));
    }

    #[test]
    fn slice_usize_index_store_uses_word_store() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "fill".to_string(),
                target: crate::ir::CallTarget::same_file("fill".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreUsizeToSliceIndex {
                        destination: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                        value: UsizeValue::Const(42),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 3),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X2, XReg::X0, 0)
        ));
    }

    #[test]
    fn slice_bool_index_store_uses_byte_store() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "fill".to_string(),
                target: crate::ir::CallTarget::same_file("fill".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreBoolToSliceIndex {
                        destination: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(0),
                        value: BoolValue::Const(true),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
        ));
    }

    #[test]
    fn slice_str_index_store_uses_two_word_stores() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "fill".to_string(),
                target: crate::ir::CallTarget::same_file("fill".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreStrToSliceIndex {
                        destination: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                        value: StrValue::StaticBytes(b"arg".to_vec()),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 4),
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X2, XReg::X0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X3, XReg::X0, 8)
        ));
    }

    #[test]
    fn open_read_uses_open_syscall() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::OpenRead {
                    destination: I32Location::Return,
                    path: UsizeValue::Const(4096),
                    failure_mode: FallibleFailureMode::Trap,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_bytes(
            &code.text,
            &encoded_mov_u32_to_w(WReg::W16, DARWIN_OPEN_SYSCALL)
        ));
        assert!(contains_instruction(&code.text, [0x01, 0x10, 0x00, 0xd4])); // svc #0x80
    }

    #[test]
    fn aggregate_scalar_field_stores_to_slot_use_frame_offsets() {
        let layout = ValueLayout::new(8, 4);
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: I32Value::Const(42),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W16, 1)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_w_sp(WReg::W16, 4)
        ));
    }

    #[test]
    fn aggregate_scalar_field_loads_from_slot_use_frame_offsets() {
        let layout = ValueLayout::new(16, 8);
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 0,
                },
                Instruction::LoadAggregateBool {
                    destination: BoolLocation::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 1,
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldrb_w_sp(WReg::W0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldrb_w_sp(WReg::W0, 1)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_w_sp(WReg::W0, 4)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X0, 8)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_i32_field_load_extracts_register_word() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "code".to_string(),
                target: crate::ir::CallTarget::same_file("code".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 0 },
                        offset: 4,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_i32_field_load_after_call_reads_spilled_word() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "code".to_string(),
                target: crate::ir::CallTarget::same_file("code".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::CallVoid {
                        target: CallTarget::same_file("effect"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 0 },
                        offset: 0,
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_w(WReg::W0, WReg::W16)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_i32_field_load_reads_shifted_second_word() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "code".to_string(),
                target: crate::ir::CallTarget::same_file("code".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 2 },
                        offset: 12,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X16, XReg::X3)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_u8_field_load_masks_selected_byte() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "read".to_string(),
                target: crate::ir::CallTarget::same_file("read".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 0 },
                        offset: 2,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 16)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_u8_field_load_reads_shifted_start_register() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "read".to_string(),
                target: crate::ir::CallTarget::same_file("read".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 2 },
                        offset: 9,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X16, XReg::X3)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_u8_field_load_reads_stack_word() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "read".to_string(),
                target: crate::ir::CallTarget::same_file("read".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Return,
                        source: AggregateLocation::DirectParameter { start_index: 8 },
                        offset: 4,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
        ));
    }

    #[test]
    fn direct_aggregate_parameter_usize_field_load_reads_stack_word() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "read".to_string(),
                target: crate::ir::CallTarget::same_file("read".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::LoadAggregateUsize {
                        destination: UsizeLocation::Return,
                        source: AggregateLocation::DirectParameter { start_index: 8 },
                        offset: 8,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X0, 8)
        ));
    }

    #[test]
    fn aggregate_range_copy_from_stack_passed_direct_parameter_reads_unaligned_bytes() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 1),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(0),
                    destination_offset: 0,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    source_offset: 4,
                    layout: ValueLayout::new(5, 1),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X17, 16)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X17, 24)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_lsr_x_imm(XReg::X17, XReg::X17, 32)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W17, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W17, 4)
        ));
    }

    #[test]
    fn aggregate_range_copy_from_stack_passed_direct_parameter_after_call_uses_spills() {
        let layout = ValueLayout::new(9, 1);
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: Type::DirectAggregate { layout, words: 2 },
                instructions: vec![
                    Instruction::CallVoid {
                        target: CallTarget::same_file("effect"),
                        arguments: vec![],
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::DirectReturn,
                        destination_offset: 0,
                        source: AggregateLocation::DirectParameter { start_index: 8 },
                        source_offset: 0,
                        layout,
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X0, XReg::X16)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X17, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X1, XReg::X16)
        ));
    }

    #[test]
    fn aggregate_range_copy_to_borrowed_parameter_after_call_uses_spilled_parameter_pointer() {
        let layout = ValueLayout::new(16, 8);
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "set_header".to_string(),
                target: crate::ir::CallTarget::same_file("set_header".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: UsizeValue::Const(42),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("effect"),
                        arguments: vec![],
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::Parameter(0),
                        destination_offset: 8,
                        source: AggregateLocation::Slot(0),
                        source_offset: 0,
                        layout,
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X17, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X16, XReg::X17, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_imm(XReg::X16, XReg::X17, 16)
        ));
    }

    #[test]
    fn aggregate_copy_from_slot_to_return_copies_words_to_x8_destination() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::CallAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Return,
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 0,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X8, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X8, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 8)
        ));
        assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
    }

    #[test]
    fn aggregate_range_copy_to_direct_return_second_word_is_allowed() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::DirectAggregate {
                    layout: ValueLayout::new(16, 8),
                    words: 2,
                },
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(1, 1),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: U8Value::Const(42),
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::DirectReturn,
                        destination_offset: 8,
                        source: AggregateLocation::Slot(0),
                        source_offset: 0,
                        layout: ValueLayout::new(1, 1),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldrb_w_sp(WReg::W16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X1, XReg::X16)
        ));
    }

    #[test]
    fn aggregate_range_copy_to_direct_return_rejects_range_past_second_word() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(9, 1),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout: ValueLayout::new(9, 1),
                },
                Instruction::Return,
            ],
        }]);

        let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

        assert!(
            diagnostics[0]
                .message
                .contains("direct aggregate return range exceeds two ABI words"),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn aggregate_copy_from_slot_to_slot_copies_words_between_stack_slots() {
        let layout = ValueLayout::new(24, 8);
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 8,
                    value: UsizeValue::Const(8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 16,
                    value: UsizeValue::Const(9),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
                    layout,
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 24)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 32)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 40)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 16)
        ));
    }

    #[test]
    fn aggregate_borrow_argument_passes_slot_address() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "touch".to_string(),
                target: crate::ir::CallTarget::same_file("touch".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0xf0, 0x23, 0x00, 0x91])); // add x16, sp, #8
        assert!(contains_instruction(&code.text, [0xf0, 0x03, 0x00, 0xf9])); // str x16, [sp, #0]
        assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x40, 0xf9])); // ldr x0, [sp, #0]
    }

    #[test]
    fn direct_aggregate_argument_passes_slot_words() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(16, 8),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: usize_const(40),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: usize_const(2),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("consume"),
                        arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                            source: AggregateArgumentSource::Slot(0),
                            layout: ValueLayout::new(16, 8),
                            words: 2,
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 16)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X16, 24)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X1, 8)
        ));
    }

    #[test]
    fn direct_aggregate_argument_zero_extends_partial_final_word() {
        let layout = ValueLayout::new(9, 1);
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: usize_const(0x0102_0304_0506_0708),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: U8Value::Const(0xaa),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("consume"),
                        arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                            source: AggregateArgumentSource::Slot(0),
                            layout,
                            words: 2,
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldrb_w_sp(WReg::W16, 24)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X1, 8)
        ));
    }

    #[test]
    fn indirect_aggregate_parameter_copy_reads_from_parameter_pointer() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "length".to_string(),
                target: crate::ir::CallTarget::same_file("length".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Parameter(0),
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::LoadAggregateUsize {
                        destination: UsizeLocation::Return,
                        source: AggregateLocation::Slot(0),
                        offset: 8,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_imm(XReg::X16, XReg::X0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_imm(XReg::X16, XReg::X0, 8)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_imm(XReg::X16, XReg::X0, 16)
        ));
    }

    #[test]
    fn borrowed_aggregate_parameter_field_read_loads_from_parameter_pointer() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "code".to_string(),
                target: crate::ir::CallTarget::same_file("code".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::Parameter(0),
                        offset: 4,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_ldr_w_imm(WReg::W0, XReg::X0, 4)
        ));
    }

    #[test]
    fn readwrite_borrowed_aggregate_parameter_field_write_stores_to_parameter_pointer() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "set_code".to_string(),
                target: crate::ir::CallTarget::same_file("set_code".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Parameter(0),
                        offset: 4,
                        value: I32Value::Const(99),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W16, XReg::X0, 4)
        ));
    }

    #[test]
    fn borrowed_aggregate_parameter_field_write_after_call_uses_spilled_parameter_pointer() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "set_code".to_string(),
                target: crate::ir::CallTarget::same_file("set_code".to_string()),
                return_type: Type::Void,
                instructions: vec![
                    Instruction::CallVoid {
                        target: CallTarget::same_file("effect"),
                        arguments: vec![],
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Parameter(0),
                        offset: 4,
                        value: I32Value::Const(99),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X16, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X17, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_str_w_imm(WReg::W16, XReg::X17, 4)
        ));
    }

    #[test]
    fn direct_aggregate_call_result_stores_partial_final_word_to_slot() {
        let layout = ValueLayout::new(9, 1);
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::CallDirectAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                        layout,
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::DirectAggregate { layout, words: 2 },
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: usize_const(0x0102_0304_0506_0708),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: U8Value::Const(0xaa),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(0),
                        layout,
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X0, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W1, 8)
        ));
    }

    #[test]
    fn fallible_aggregate_call_passes_destination_slot_and_checks_status() {
        let aggregate = Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        };
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Fallible(Box::new(aggregate.clone())),
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::CallFallibleAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                        failure_mode: FallibleFailureMode::Propagate,
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Return,
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(24, 8),
                    },
                    Instruction::ReturnFallibleSuccess,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Fallible(Box::new(aggregate)),
                instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 0,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::ReturnFallibleSuccess,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0xe8, 0x23, 0x00, 0x91])); // add x8, sp, #8
        assert!(contains_instruction(
            &code.text,
            encoded_ldr_x_sp(XReg::X8, 0)
        ));
        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
    }

    #[test]
    fn fallible_direct_aggregate_call_stores_success_payload_words() {
        let aggregate = Type::DirectAggregate {
            layout: ValueLayout::new(16, 8),
            words: 2,
        };
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Fallible(Box::new(Type::I32)),
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(16, 8),
                    },
                    Instruction::CallFallibleDirectAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                        layout: ValueLayout::new(16, 8),
                        failure_mode: FallibleFailureMode::Propagate,
                    },
                    set_return_i32(0),
                    Instruction::ReturnFallibleSuccess,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Fallible(Box::new(aggregate)),
                instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 0,
                        value: UsizeValue::Const(7),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 8,
                        value: UsizeValue::Const(9),
                    },
                    Instruction::ReturnFallibleSuccess,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(&code.text, [0xe1, 0x03, 0x00, 0xf9])); // str x1, [sp, #0]
        assert!(contains_instruction(&code.text, [0xe2, 0x07, 0x00, 0xf9])); // str x2, [sp, #8]
    }

    #[test]
    fn fallible_direct_aggregate_call_stores_partial_final_payload_word() {
        let layout = ValueLayout::new(9, 1);
        let aggregate = Type::DirectAggregate { layout, words: 2 };
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Fallible(Box::new(Type::I32)),
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::CallFallibleDirectAggregate {
                        destination: AggregateLocation::Slot(0),
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                        layout,
                        failure_mode: FallibleFailureMode::Propagate,
                    },
                    set_return_i32(0),
                    Instruction::ReturnFallibleSuccess,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Fallible(Box::new(aggregate)),
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: usize_const(0x0102_0304_0506_0708),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: U8Value::Const(0xaa),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(0),
                        layout,
                    },
                    Instruction::ReturnFallibleSuccess,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(
            &code.text,
            encoded_str_x_sp(XReg::X1, 0)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_strb_w_sp(WReg::W2, 8)
        ));
    }

    #[test]
    fn fallible_direct_aggregate_call_forwards_partial_payload_to_direct_return() {
        let layout = ValueLayout::new(9, 1);
        let aggregate = Type::DirectAggregate { layout, words: 2 };
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(0), Instruction::Return],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: aggregate.clone(),
                instructions: vec![
                    Instruction::CallFallibleDirectAggregate {
                        destination: AggregateLocation::DirectReturn,
                        target: CallTarget::same_file("make"),
                        arguments: vec![],
                        layout,
                        failure_mode: FallibleFailureMode::Trap,
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "make".to_string(),
                target: crate::ir::CallTarget::same_file("make".to_string()),
                return_type: Type::Fallible(Box::new(aggregate)),
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: usize_const(0x0102_0304_0506_0708),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 8,
                        value: U8Value::Const(0xaa),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(0),
                        layout,
                    },
                    Instruction::ReturnFallibleSuccess,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X0, XReg::X1)
        ));
        assert!(contains_instruction(
            &code.text,
            encoded_mov_x(XReg::X1, XReg::X2)
        ));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_i32_normal_call_stages_reordered_arguments() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("wrapper", vec![i32_const(5), i32_const(42)])],
            },
            Function {
                name: "wrapper".to_string(),
                target: crate::ir::CallTarget::same_file("wrapper".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(
                        I32Location::Local(0),
                        "second",
                        vec![i32_param(1), i32_param(0)],
                    ),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "second".to_string(),
                target: crate::ir::CallTarget::same_file("second".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-reordered-normal-call-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(5));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_str_literal_argument_uses_two_abi_words() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        ScalarArgument::Str(StrValue::StaticBytes(b"Nocter".to_vec())),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                }],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(2),
                    },
                    Instruction::Return,
                ],
            },
        ]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        assert_eq!(code.read_only_data, b"Nocter");
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-str-argument-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(42));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_bool_normal_call_preserves_local_condition() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(7), Instruction::Return],
                        else_instructions: vec![set_return_i32(9), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "ready".to_string(),
                target: crate::ir::CallTarget::same_file("ready".to_string()),
                return_type: Type::Bool,
                instructions: vec![set_return_bool(true), Instruction::Return],
            },
        ]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-bool-normal-call-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(7));
    }

    #[test]
    fn generates_exit_code_for_return_bool_true() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Const(true),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_same_file_function_call() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x01, 0x00, 0x00, 0x14, // b answer
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_i32_tail_call_with_arguments_and_add() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
                target: crate::ir::CallTarget::same_file("add".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xff, 0x83, 0x00, 0xd1, // sub sp, sp, #32
                0xfe, 0x0f, 0x00, 0xf9, // str x30, [sp, #24]
                0x90, 0x02, 0x80, 0x52, // movz w16, #20
                0xf0, 0x03, 0x00, 0xb9, // str w16, [sp, #0]
                0xd0, 0x02, 0x80, 0x52, // movz w16, #22
                0xf0, 0x0b, 0x00, 0xb9, // str w16, [sp, #8]
                0xe0, 0x03, 0x40, 0xb9, // ldr w0, [sp, #0]
                0xe1, 0x0b, 0x40, 0xb9, // ldr w1, [sp, #8]
                0xfe, 0x0f, 0x40, 0xf9, // ldr x30, [sp, #24]
                0xff, 0x83, 0x00, 0x91, // add sp, sp, #32
                0x01, 0x00, 0x00, 0x14, // b add
                0xf0, 0x03, 0x00, 0x2a, // mov w16, w0
                0xe0, 0x03, 0x01, 0x2a, // mov w0, w1
                0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
                0x47, 0x00, 0x00, 0x54, // b.vc +8
                0x00, 0x00, 0x20, 0xd4, // brk #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_i32_local_binding_return() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x49, 0x05, 0x80, 0x52, // movz w9, #42
                0xe0, 0x03, 0x09, 0x2a, // mov w0, w9
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_i32_local_addition_binding_return() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(40),
                },
                Instruction::AddI32 {
                    destination: I32Location::Local(1),
                    left: i32_local(0),
                    right: i32_const(2),
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(1),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x09, 0x05, 0x80, 0x52, // movz w9, #40
                0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
                0x4a, 0x00, 0x80, 0x52, // movz w10, #2
                0x0a, 0x02, 0x0a, 0x2b, // adds w10, w16, w10
                0x47, 0x00, 0x00, 0x54, // b.vc +8
                0x00, 0x00, 0x20, 0xd4, // brk #0
                0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_i32_addition_with_overflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_const(40),
                    right: i32_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x2b])); // adds w0, w16, w0
        assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    }

    #[test]
    fn generates_i32_subtraction_with_overflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SubtractI32 {
                    destination: I32Location::Return,
                    left: i32_const(40),
                    right: i32_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x6b])); // subs w0, w16, w0
        assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    }

    #[test]
    fn generates_i32_multiplication_with_overflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::MultiplyI32 {
                    destination: I32Location::Return,
                    left: i32_const(21),
                    right: i32_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x11, 0x7e, 0x20, 0x9b])); // smull x17, w16, w0
        assert!(contains_instruction(&code.text, [0x30, 0x7e, 0x40, 0x93])); // sxtw x16, w17
        assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x10, 0xeb])); // cmp x17, x16
        assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    }

    #[test]
    fn generates_i32_shift_left_with_count_traps() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ShiftLeftI32 {
                    destination: I32Location::Return,
                    left: i32_const(5),
                    right: i32_const(3),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
        assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x1a])); // lslv w0, w16, w0
    }

    #[test]
    fn generates_i32_shift_right_with_count_traps() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ShiftRightI32 {
                    destination: I32Location::Return,
                    left: i32_const(8),
                    right: i32_const(1),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
        assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x2a, 0xc0, 0x1a])); // asrv w0, w16, w0
    }

    #[test]
    fn generates_i32_division_with_safety_traps() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::DivideI32 {
                    destination: I32Location::Return,
                    left: i32_const(84),
                    right: i32_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x0e, 0xc0, 0x1a])); // sdiv w0, w16, w0
    }

    #[test]
    fn generates_i32_remainder_with_safety_traps() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::RemainderI32 {
                    destination: I32Location::Return,
                    left: i32_const(85),
                    right: i32_const(43),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x11, 0x0e, 0xc0, 0x1a])); // sdiv w17, w16, w0
        assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x1b])); // msub w0, w17, w0, w16
    }

    #[test]
    fn generates_usize_addition_with_overflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::AddUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(40),
                    right: usize_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xab])); // adds x0, x16, x0
        assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    }

    #[test]
    fn generates_usize_subtraction_with_underflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SubtractUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(40),
                    right: usize_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xeb])); // subs x0, x16, x0
        assert!(contains_instruction(&code.text, [0x42, 0x00, 0x00, 0x54])); // b.cs +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    }

    #[test]
    fn generates_usize_multiplication_with_overflow_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::MultiplyUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(21),
                    right: usize_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x11, 0x7e, 0xc0, 0x9b])); // umulh x17, x16, x0
        assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x1f, 0xeb])); // cmp x17, xzr
        assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x7e, 0x00, 0x9b])); // mul x0, x16, x0
    }

    #[test]
    fn generates_usize_division_with_zero_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::DivideUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(84),
                    right: usize_const(2),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x0a, 0xc0, 0x9a])); // udiv x0, x16, x0
    }

    #[test]
    fn generates_usize_remainder_with_zero_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::RemainderUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(85),
                    right: usize_const(43),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
        assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x11, 0x0a, 0xc0, 0x9a])); // udiv x17, x16, x0
        assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x9b])); // msub x0, x17, x0, x16
    }

    #[test]
    fn generates_usize_shift_left_with_count_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::ShiftLeftUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(5),
                    right: usize_const(3),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
        assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x9a])); // lslv x0, x16, x0
    }

    #[test]
    fn generates_usize_shift_right_with_count_trap() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::ShiftRightUsize {
                    destination: UsizeLocation::Return,
                    left: usize_const(8),
                    right: usize_const(1),
                },
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
        assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x26, 0xc0, 0x9a])); // lsrv x0, x16, x0
    }

    #[test]
    fn generates_terminal_if_with_false_condition() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(false),
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(2), Instruction::Return],
            }],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x03, 0x00, 0x00, 0x14, // b else
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0x40, 0x00, 0x80, 0x52, // movz w0, #2
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_while_with_false_condition_and_backward_branch() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::While {
                    condition_instructions: vec![],
                    condition: BoolValue::Const(false),
                    body_instructions: vec![set_return_i32(1)],
                },
                set_return_i32(2),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x03, 0x00, 0x00, 0x14, // b while end
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xfe, 0xff, 0xff, 0x17, // b while condition
                0x40, 0x00, 0x80, 0x52, // movz w0, #2
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_while_break_to_loop_end() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::While {
                    condition_instructions: vec![],
                    condition: BoolValue::Const(true),
                    body_instructions: vec![Instruction::Break],
                },
                set_return_i32(2),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x01, 0x00, 0x00, 0x14, // b while end
                0x40, 0x00, 0x80, 0x52, // movz w0, #2
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_while_continue_to_loop_start() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::While {
                    condition_instructions: vec![],
                    condition: BoolValue::Const(true),
                    body_instructions: vec![Instruction::Continue],
                },
                set_return_i32(2),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x00, 0x00, 0x00, 0x14, // b while condition
                0x40, 0x00, 0x80, 0x52, // movz w0, #2
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_terminal_if_with_bool_local_condition() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![set_return_i32(7), Instruction::Return],
                    else_instructions: vec![set_return_i32(9), Instruction::Return],
                },
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x29, 0x00, 0x80, 0x52, // movz w9, #1
                0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
                0x1f, 0x02, 0x1f, 0x6b, // cmp w16, #0
                0x60, 0x00, 0x00, 0x54, // b.eq else
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0x20, 0x01, 0x80, 0x52, // movz w0, #9
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_terminal_if_returning_bool() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Bool,
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(false),
                then_instructions: vec![set_return_bool(true), Instruction::Return],
                else_instructions: vec![set_return_bool(false), Instruction::Return],
            }],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x03, 0x00, 0x00, 0x14, // b else
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_terminal_if_with_i32_equality_condition() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: i32_const(1),
                    right: i32_const(1),
                },
                then_instructions: vec![set_return_i32(7), Instruction::Return],
                else_instructions: vec![set_return_i32(9), Instruction::Return],
            }],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x31, 0x00, 0x80, 0x52, // movz w17, #1
                0x1f, 0x02, 0x11, 0x6b, // cmp w16, w17
                0x61, 0x00, 0x00, 0x54, // b.ne else
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0x20, 0x01, 0x80, 0x52, // movz w0, #9
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn generates_terminal_if_with_i32_less_condition() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_const(1),
                    right: i32_const(2),
                },
                then_instructions: vec![set_return_i32(7), Instruction::Return],
                else_instructions: vec![set_return_i32(9), Instruction::Return],
            }],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x51, 0x00, 0x80, 0x52, // movz w17, #2
                0x1f, 0x02, 0x11, 0x6b, // cmp w16, w17
                0x6a, 0x00, 0x00, 0x54, // b.ge else
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
                0x20, 0x01, 0x80, 0x52, // movz w0, #9
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn maps_i32_comparison_operators_to_arm64_conditions() {
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::Equal),
            BranchCondition::Eq
        );
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::NotEqual),
            BranchCondition::Ne
        );
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::Less),
            BranchCondition::Lt
        );
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::LessEqual),
            BranchCondition::Le
        );
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::Greater),
            BranchCondition::Gt
        );
        assert_eq!(
            branch_condition_for_true_comparison(I32ComparisonOperator::GreaterEqual),
            BranchCondition::Ge
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_str_write_runs() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStr {
                    fd: I32Value::Const(1),
                    text: StrValue::StaticBytes(b"hello\n".to_vec()),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-str-write-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"hello\n");
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_slice_write_runs() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteSlice {
                    fd: I32Value::Const(1),
                    bytes: SliceValue::StrBytes(StrValue::StaticBytes(b"bytes\n".to_vec())),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-slice-write-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"bytes\n");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn write_str_emits_loop_for_partial_writes() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStr {
                    fd: I32Value::Const(1),
                    text: StrValue::StaticBytes(b"hello\n".to_vec()),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert!(
            contains_backward_unconditional_branch(&code.text),
            "write lowering should loop until the requested byte count is exhausted"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_close_fd_closes_stdout() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CloseFd {
                    fd: I32Value::Const(1),
                },
                Instruction::WriteStr {
                    fd: I32Value::Const(1),
                    text: StrValue::StaticBytes(b"hidden\n".to_vec()),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-close-fd-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_read_zero_bytes_runs() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReadSlice {
                    destination: UsizeLocation::Local(0),
                    fd: I32Value::Const(0),
                    buffer: SliceValue::StrBytes(StrValue::StaticBytes(Vec::new())),
                    failure_mode: FallibleFailureMode::Trap,
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module).unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-read-zero-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn generates_exit_zero_for_return_void() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module).unwrap();

        assert_eq!(
            code.text,
            vec![
                0x05, 0x00, 0x00, 0x94, // bl main
                0x00, 0x00, 0x80, 0x52, // movz w0, #0
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn write_temp_executable(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!(
            "nocter-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let executable = std::env::temp_dir().join(unique);
        std::fs::write(&executable, bytes).unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        executable
    }

    fn set_return_i32(value: i32) -> Instruction {
        Instruction::SetI32 {
            destination: I32Location::Return,
            value: i32_const(value),
        }
    }

    fn set_return_usize(value: u64) -> Instruction {
        Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: usize_const(value),
        }
    }

    fn set_return_bool(value: bool) -> Instruction {
        Instruction::SetBool {
            destination: BoolLocation::Return,
            value: BoolValue::Const(value),
        }
    }

    fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
        Instruction::TailCall {
            target: CallTarget::same_file(function),
            arguments: i32_arguments(arguments),
        }
    }

    fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
        Instruction::CallI32 {
            destination,
            target: CallTarget::same_file(function),
            arguments: i32_arguments(arguments),
        }
    }

    fn call_void(function: &str, arguments: Vec<I32Value>) -> Instruction {
        Instruction::CallVoid {
            target: CallTarget::same_file(function),
            arguments: i32_arguments(arguments),
        }
    }

    fn call_bool(
        destination: BoolLocation,
        function: &str,
        arguments: Vec<I32Value>,
    ) -> Instruction {
        Instruction::CallBool {
            destination,
            target: CallTarget::same_file(function),
            arguments: i32_arguments(arguments),
        }
    }

    fn i32_arguments(arguments: Vec<I32Value>) -> Vec<ScalarArgument> {
        arguments.into_iter().map(ScalarArgument::I32).collect()
    }

    fn i32_const(value: i32) -> I32Value {
        I32Value::Const(value)
    }

    fn i32_param(index: usize) -> I32Value {
        I32Value::Location(I32Location::Parameter(index))
    }

    fn i32_local(index: usize) -> I32Value {
        I32Value::Location(I32Location::Local(index))
    }

    fn readonly_u8_slice_type() -> Type {
        Type::Slice {
            is_readwrite: false,
        }
    }

    fn usize_const(value: u64) -> UsizeValue {
        UsizeValue::Const(value)
    }

    fn contains_instruction(text: &[u8], instruction: [u8; 4]) -> bool {
        text.windows(4).any(|window| window == instruction)
    }

    fn instruction_offset(text: &[u8], instruction: &[u8]) -> Option<usize> {
        text.windows(instruction.len())
            .position(|window| window == instruction)
    }

    fn contains_backward_unconditional_branch(text: &[u8]) -> bool {
        text.chunks_exact(4).any(|chunk| {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());
            let is_unconditional_branch = (word & 0xfc00_0000) == 0x1400_0000;
            let offset_is_negative = (word & 0x0200_0000) != 0;
            is_unconditional_branch && offset_is_negative
        })
    }

    fn contains_bytes(text: &[u8], bytes: &[u8]) -> bool {
        text.windows(bytes.len()).any(|window| window == bytes)
    }

    fn encoded_mov_u32_to_w(register: WReg, value: u32) -> Vec<u8> {
        let mut encoder = Encoder::new();
        emit_mov_u32_to_w(&mut encoder, register, value);
        encoder.finish()
    }

    fn encoded_ldr_x_sp(register: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldr_x_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_ldr_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldr_x_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_mov_x(destination: XReg, source: XReg) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_mov_x(destination, source);
        encoded_instruction(encoder)
    }

    fn encoded_mov_w(destination: WReg, source: WReg) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_mov_w(destination, source);
        encoded_instruction(encoder)
    }

    fn encoded_lsl_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_lsl_x_imm(destination, source, shift);
        encoded_instruction(encoder)
    }

    fn encoded_adds_x(destination: XReg, left: XReg, right: XReg) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_adds_x(destination, left, right);
        encoded_instruction(encoder)
    }

    fn encoded_lsr_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_lsr_x_imm(destination, source, shift);
        encoded_instruction(encoder)
    }

    fn encoded_ldr_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldr_w_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_ldr_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldr_w_reg(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_ldr_w_sp(register: WReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldr_w_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_ldrb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldrb_w_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_ldrb_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_ldrb_w_reg(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_str_x_sp(register: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_str_x_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_str_w_sp(register: WReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_str_w_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_strb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_strb_w_sp(register, offset);
        encoded_instruction(encoder)
    }

    fn encoded_str_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_str_w_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_strb_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_strb_w_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_strh_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_strh_w_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_str_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
        let mut encoder = Encoder::new();
        encoder.emit_str_x_imm(register, base, offset);
        encoded_instruction(encoder)
    }

    fn encoded_instruction(encoder: Encoder) -> [u8; 4] {
        encoder.finish().try_into().unwrap()
    }
}
