use crate::backend::frame::{FrameLayout, FunctionFrame, plan_function_frame};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    CallTarget, FallibleFailureMode, Function, I32Value, Instruction, IrModule, StrValue, Type,
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
    entry_name: &str,
) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut emitter = EntryEmitter::new();
    emitter.emit_module(module, entry_name)?;
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
        }
    }

    fn emit_module(&mut self, module: &IrModule, entry_name: &str) -> Result<(), Vec<Diagnostic>> {
        let Some(entry) = module
            .functions
            .iter()
            .find(|function| function.name == entry_name)
        else {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!("codegen requires a lowered entry function `{entry_name}`"),
            )]);
        };

        self.emit_process_entry(entry)?;

        for function in &module.functions {
            self.emit_function(function)?;
        }

        Ok(())
    }

    fn emit_process_entry(&mut self, entry: &Function) -> Result<(), Vec<Diagnostic>> {
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
        let frame = match frame {
            FunctionFrame::Frameless => None,
            FunctionFrame::Framed(layout) => {
                self.emit_prologue(layout);
                Some(layout)
            }
        };

        for instruction in &function.instructions {
            self.emit_instruction(instruction, frame, &function.return_type)?;
        }

        Ok(())
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
            Instruction::SetSlice { destination, value } => {
                self.emit_set_slice(*destination, value)?;
            }
            Instruction::ReserveAggregateSlot { .. } => {}
            Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            } => {
                self.emit_store_aggregate_usize(*destination, *offset, value, frame)?;
            }
            Instruction::CopyAggregate {
                destination,
                source,
                layout,
            } => {
                self.emit_copy_aggregate(*destination, *source, *layout, frame)?;
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
            Instruction::ReturnFallibleFailure { code, message } => {
                self.emit_return_fallible_failure(code, message, frame)?;
            }
            Instruction::Return => {
                self.emit_return(frame);
            }
        }

        Ok(())
    }

    fn emit_fallible_process_exit(&mut self, success_type: &Type) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Ne);

        match success_type {
            Type::I32 => {
                self.encoder.emit_mov_w(WReg::W0, WReg::W1);
            }
            Type::Void => {
                emit_mov_i32_to_w0(&mut self.encoder, 0);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "codegen only supports `i32!` and `void!` executable entry returns",
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
            Type::Void => {}
            Type::Aggregate { .. } | Type::Borrow { .. } | Type::Never | Type::Fallible(_) => {
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
        self.emit_str_value_to_x_pair(code, XReg::X1, XReg::X2)?;
        self.emit_str_value_to_x_pair(message, XReg::X3, XReg::X4)?;
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        self.emit_return(frame);
        Ok(())
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
            FallibleFailureMode::Trap => {
                self.emit_trap();
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

    fn emit_prologue(&mut self, frame: &FrameLayout) {
        self.encoder.emit_sub_sp_imm(frame.frame_size());
        self.encoder
            .emit_str_x_sp(XReg::X30, frame.saved_x30_offset());
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
        self.encoder.emit_mov_x(XReg::X1, XReg::X3);
        self.encoder.emit_mov_x(XReg::X2, XReg::X4);
        emit_darwin_write_syscall(&mut self.encoder);
        self.emit_scalar_reloads(frame)?;
        let failure_branch = self.emit_cond_branch_placeholder(BranchCondition::Cs);
        emit_mov_i32_to_w0(&mut self.encoder, 0);
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(failure_branch, "write syscall failure target")?;
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(WRITE_FAILURE_CODE.to_vec()),
            XReg::X1,
            XReg::X2,
        )?;
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(WRITE_FAILURE_MESSAGE.to_vec()),
            XReg::X3,
            XReg::X4,
        )?;
        emit_mov_i32_to_w0(&mut self.encoder, 1);

        self.patch_branch_placeholder_to_current(end_branch, "write syscall end target")?;
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

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

const STDERR_FILENO: u64 = 2;
const FALLIBLE_REPORT_FRAME_SIZE: u32 = 32;
const WRITE_FAILURE_CODE: &[u8] = b"std.io.write_failed";
const WRITE_FAILURE_MESSAGE: &[u8] = b"write failed";
const ADR_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i64 = (1 << 20) - 1;
const BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 27);
const BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 27) - 4;
const DARWIN_WRITE_SYSCALL: u32 = 0x0200_0004;
const DARWIN_EXIT_SYSCALL: u32 = 0x0200_0001;
const DARWIN_SYSCALL_TRAP: u16 = 0x80;
const I32_BIT_WIDTH: i32 = 32;
const USIZE_BIT_WIDTH: u64 = 64;

#[cfg(test)]
mod tests {
    use super::control_flow::branch_condition_for_true_comparison;
    use super::*;
    use crate::abi::ValueLayout;
    use crate::ir::{
        AggregateLocation, BoolLocation, BoolValue, CallTarget, Function, I32ComparisonOperator,
        I32Location, I32Value, ScalarArgument, SliceLocation, SliceValue, StrLocation, StrValue,
        Type, U8Location, U8Value, UsizeLocation, UsizeValue,
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
        assert!(contains_instruction(&code.text, [0x00, 0x68, 0x70, 0x38])); // ldrb w0, [x0, x16]
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(contains_instruction(&code.text, [0x00, 0x68, 0x70, 0x38])); // ldrb w0, [x0, x16]
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(contains_instruction(&code.text, [0x10, 0x68, 0x70, 0x38])); // ldrb w16, [x0, x16]
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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
                return_type: Type::Void,
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
    }

    #[test]
    fn aggregate_return_call_keeps_existing_x8_destination() {
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
                return_type: Type::Void,
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(!contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert!(contains_instruction(&code.text, [0xf0, 0x03, 0x40, 0xf9])); // ldr x16, [sp, #0]
        assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
        assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
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
        let code = generate_arm64_darwin_entry(&module, "main").unwrap();
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
        let code = generate_arm64_darwin_entry(&module, "main").unwrap();
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
        let code = generate_arm64_darwin_entry(&module, "main").unwrap();
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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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
        let code = generate_arm64_darwin_entry(&module, "main").unwrap();
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

    #[test]
    fn generates_exit_zero_for_return_void() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

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
}
