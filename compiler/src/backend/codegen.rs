use crate::backend::frame::{FrameLayout, FunctionFrame, plan_function_frame};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, CallTarget, Function,
    I32ComparisonOperator, I32Location, I32Value, Instruction, IrModule, Type,
};
use crate::target::arm64::{BranchCondition, Encoder, MoveWideShift, WReg, XReg};
use std::collections::HashMap;

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

        self.emit_process_entry(entry);

        for function in &module.functions {
            self.emit_function(function)?;
        }

        Ok(())
    }

    fn emit_process_entry(&mut self, entry: &Function) {
        self.emit_call(FunctionSymbol::from_function(entry));
        if matches!(entry.return_type.success_type(), Type::Void) {
            emit_mov_i32_to_w0(&mut self.encoder, 0);
        }
        emit_darwin_exit_syscall(&mut self.encoder);
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
            self.emit_instruction(instruction, frame)?;
        }

        Ok(())
    }

    fn emit_instruction(
        &mut self,
        instruction: &Instruction,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        match instruction {
            Instruction::WriteStaticStderr(bytes) => {
                self.emit_write_static_stderr(bytes);
            }
            Instruction::SetI32 { destination, value } => {
                self.emit_set_i32(*destination, value)?;
            }
            Instruction::SetBool { destination, value } => {
                self.emit_set_bool(*destination, value)?;
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
            Instruction::TailCall { target, arguments } => {
                self.emit_tail_call(FunctionSymbol::from_call_target(target), arguments, frame)?;
            }
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => {
                self.emit_if(condition, then_instructions, else_instructions, frame)?;
            }
            Instruction::Return => {
                self.emit_return(frame);
            }
        }

        Ok(())
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

    fn emit_if(
        &mut self,
        condition: &BoolValue,
        then_instructions: &[Instruction],
        else_instructions: &[Instruction],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let branches_to_else = self.emit_bool_false_branch_placeholders(condition)?;

        for instruction in then_instructions {
            self.emit_instruction(instruction, frame)?;
        }

        let branch_to_end =
            if else_instructions.is_empty() || instruction_list_ends_execution(then_instructions) {
                None
            } else {
                Some(self.emit_branch_placeholder())
            };

        self.patch_branch_placeholders_to_current(branches_to_else, "if branch target")?;

        for instruction in else_instructions {
            self.emit_instruction(instruction, frame)?;
        }

        if let Some(branch) = branch_to_end {
            self.patch_branch_placeholder_to_current(branch, "if end target")?;
        }

        Ok(())
    }

    fn emit_bool_false_branch_placeholders(
        &mut self,
        value: &BoolValue,
    ) -> Result<Vec<BranchPatch>, Vec<Diagnostic>> {
        match value {
            BoolValue::Const(true) => Ok(Vec::new()),
            BoolValue::Const(false) => Ok(vec![self.emit_branch_placeholder()]),
            BoolValue::Location(_) => {
                self.emit_bool_value_to_w(value, WReg::W16)?;
                self.encoder.emit_cmp_w_zero(WReg::W16);
                Ok(vec![self.emit_cond_branch_placeholder(BranchCondition::Eq)])
            }
            BoolValue::I32Comparison {
                operator,
                left,
                right,
            } => {
                self.emit_i32_value_to_w(left, WReg::W16)?;
                self.emit_i32_value_to_w(right, WReg::W17)?;
                self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_false_comparison(*operator),
                )])
            }
            BoolValue::BoolComparison {
                operator,
                left,
                right,
            } => {
                self.emit_bool_value_to_w(left, WReg::W16)?;
                self.emit_bool_value_to_w(right, WReg::W17)?;
                self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_false_bool_comparison(*operator),
                )])
            }
            BoolValue::Not(inner) => self.emit_bool_true_branch_placeholders(inner),
            BoolValue::Logical {
                operator,
                left,
                right,
            } => match operator {
                BoolLogicalOperator::And => {
                    let mut branches = self.emit_bool_false_branch_placeholders(left)?;
                    branches.extend(self.emit_bool_false_branch_placeholders(right)?);
                    Ok(branches)
                }
                BoolLogicalOperator::Or => {
                    let left_true_branches = self.emit_bool_true_branch_placeholders(left)?;
                    let right_false_branches = self.emit_bool_false_branch_placeholders(right)?;
                    self.patch_branch_placeholders_to_current(
                        left_true_branches,
                        "bool OR true target",
                    )?;
                    Ok(right_false_branches)
                }
            },
        }
    }

    fn emit_bool_true_branch_placeholders(
        &mut self,
        value: &BoolValue,
    ) -> Result<Vec<BranchPatch>, Vec<Diagnostic>> {
        match value {
            BoolValue::Const(true) => Ok(vec![self.emit_branch_placeholder()]),
            BoolValue::Const(false) => Ok(Vec::new()),
            BoolValue::Location(_) => {
                self.emit_bool_value_to_w(value, WReg::W16)?;
                self.encoder.emit_cmp_w_zero(WReg::W16);
                Ok(vec![self.emit_cond_branch_placeholder(BranchCondition::Ne)])
            }
            BoolValue::I32Comparison {
                operator,
                left,
                right,
            } => {
                self.emit_i32_value_to_w(left, WReg::W16)?;
                self.emit_i32_value_to_w(right, WReg::W17)?;
                self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_true_comparison(*operator),
                )])
            }
            BoolValue::BoolComparison {
                operator,
                left,
                right,
            } => {
                self.emit_bool_value_to_w(left, WReg::W16)?;
                self.emit_bool_value_to_w(right, WReg::W17)?;
                self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_true_bool_comparison(*operator),
                )])
            }
            BoolValue::Not(inner) => self.emit_bool_false_branch_placeholders(inner),
            BoolValue::Logical {
                operator,
                left,
                right,
            } => match operator {
                BoolLogicalOperator::And => {
                    let left_false_branches = self.emit_bool_false_branch_placeholders(left)?;
                    let right_true_branches = self.emit_bool_true_branch_placeholders(right)?;
                    self.patch_branch_placeholders_to_current(
                        left_false_branches,
                        "bool AND false target",
                    )?;
                    Ok(right_true_branches)
                }
                BoolLogicalOperator::Or => {
                    let mut branches = self.emit_bool_true_branch_placeholders(left)?;
                    branches.extend(self.emit_bool_true_branch_placeholders(right)?);
                    Ok(branches)
                }
            },
        }
    }

    fn emit_branch_placeholder(&mut self) -> BranchPatch {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_b(0);
        BranchPatch::Unconditional { instruction_offset }
    }

    fn emit_cond_branch_placeholder(&mut self, condition: BranchCondition) -> BranchPatch {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_b_cond(condition, 0);
        BranchPatch::Conditional {
            instruction_offset,
            condition,
        }
    }

    fn patch_branch_placeholders_to_current(
        &mut self,
        branches: Vec<BranchPatch>,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        for branch in branches {
            self.patch_branch_placeholder_to_current(branch, target_description)?;
        }

        Ok(())
    }

    fn patch_branch_placeholder_to_current(
        &mut self,
        branch: BranchPatch,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        match branch {
            BranchPatch::Unconditional { instruction_offset } => {
                self.patch_branch_to_current(instruction_offset, target_description)
            }
            BranchPatch::Conditional {
                instruction_offset,
                condition,
            } => {
                self.patch_cond_branch_to_current(instruction_offset, condition, target_description)
            }
        }
    }

    fn patch_branch_to_current(
        &mut self,
        instruction_offset: usize,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_offset = self.encoder.position() as i64 - instruction_offset as i64;
        if !(BRANCH_MIN_BYTE_OFFSET..=BRANCH_MAX_BYTE_OFFSET).contains(&byte_offset) {
            return Err(vec![Diagnostic::error(
                "E9001",
                format!("{target_description} is too far for ARM64 `b`"),
            )]);
        }

        self.encoder.patch_b(instruction_offset, byte_offset as i32);
        Ok(())
    }

    fn patch_cond_branch_to_current(
        &mut self,
        instruction_offset: usize,
        condition: BranchCondition,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_offset = self.encoder.position() as i64 - instruction_offset as i64;
        if !(COND_BRANCH_MIN_BYTE_OFFSET..=COND_BRANCH_MAX_BYTE_OFFSET).contains(&byte_offset) {
            return Err(vec![Diagnostic::error(
                "E9001",
                format!("{target_description} is too far for ARM64 `b.cond`"),
            )]);
        }

        self.encoder
            .patch_b_cond(instruction_offset, condition, byte_offset as i32);
        Ok(())
    }

    fn emit_call(&mut self, function: FunctionSymbol) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_bl(0);
        self.call_patches.push(FunctionCallPatch {
            instruction_offset,
            function,
        });
    }

    fn emit_tail_call(
        &mut self,
        function: FunctionSymbol,
        arguments: &[I32Value],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !arguments.is_empty() {
            let Some(frame) = frame else {
                return Err(vec![Diagnostic::error(
                    "E9005",
                    "tail call argument staging requires a stack frame",
                )]);
            };
            self.emit_staged_i32_arguments(arguments, frame)?;
        }

        if let Some(frame) = frame {
            self.emit_epilogue(frame);
        }

        let instruction_offset = self.encoder.position();
        self.encoder.emit_b(0);
        self.tail_call_patches.push(FunctionCallPatch {
            instruction_offset,
            function,
        });

        Ok(())
    }

    fn emit_call_i32(
        &mut self,
        destination: I32Location,
        function: FunctionSymbol,
        arguments: &[I32Value],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal i32 call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_staged_i32_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_i32_location(destination)
    }

    fn emit_call_bool(
        &mut self,
        destination: BoolLocation,
        function: FunctionSymbol,
        arguments: &[I32Value],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal bool call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_staged_i32_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_bool_location(destination)
    }

    fn emit_staged_i32_arguments(
        &mut self,
        arguments: &[I32Value],
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for (index, argument) in arguments.iter().enumerate() {
            let Some(slot) = frame.argument_staging_slots().get(index) else {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 i32 arguments, got argument {index}"),
                )]);
            };
            debug_assert_eq!(slot.argument_index(), index);
            self.emit_i32_value_to_w(argument, WReg::W16)?;
            self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
        }

        for slot in frame.argument_staging_slots().iter().take(arguments.len()) {
            let Some(register) = WReg::argument(slot.argument_index()) else {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    format!(
                        "codegen supports at most 8 i32 arguments, got argument {}",
                        slot.argument_index()
                    ),
                )]);
            };
            self.encoder.emit_ldr_w_sp(register, slot.offset());
        }

        Ok(())
    }

    fn emit_scalar_spills(&mut self, frame: &FrameLayout) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            let register = WReg::local(slot.local_index()).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!(
                        "codegen supports at most 7 local scalar bindings, got local {}",
                        slot.local_index()
                    ),
                )]
            })?;
            self.encoder.emit_str_w_sp(register, slot.offset());
        }

        Ok(())
    }

    fn emit_scalar_reloads(&mut self, frame: &FrameLayout) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            let register = WReg::local(slot.local_index()).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!(
                        "codegen supports at most 7 local scalar bindings, got local {}",
                        slot.local_index()
                    ),
                )]
            })?;
            self.encoder.emit_ldr_w_sp(register, slot.offset());
        }

        Ok(())
    }

    fn emit_call_result_to_i32_location(
        &mut self,
        destination: I32Location,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        if destination != WReg::W0 {
            self.encoder.emit_mov_w(destination, WReg::W0);
        }

        Ok(())
    }

    fn emit_call_result_to_bool_location(
        &mut self,
        destination: BoolLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.bool_location_register(destination)?;
        if destination != WReg::W0 {
            self.encoder.emit_mov_w(destination, WReg::W0);
        }

        Ok(())
    }

    fn emit_set_i32(
        &mut self,
        destination: I32Location,
        value: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(value, destination)
    }

    fn emit_set_bool(
        &mut self,
        destination: BoolLocation,
        value: &BoolValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.bool_location_register(destination)?;
        self.emit_bool_value_to_w(value, destination)
    }

    fn emit_add_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_adds_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 addition non-overflow target")?;
        Ok(())
    }

    fn emit_subtract_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_subs_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 subtraction non-overflow target")?;
        Ok(())
    }

    fn emit_multiply_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder.emit_smull_x(XReg::X17, WReg::W16, destination);
        self.encoder.emit_sxtw_x_w(XReg::X16, WReg::W17);
        self.encoder.emit_cmp_x(XReg::X17, XReg::X16);
        let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(exact_fit, "i32 multiplication exact-fit target")?;
        if destination != WReg::W17 {
            self.encoder.emit_mov_w(destination, WReg::W17);
        }
        Ok(())
    }

    fn emit_divide_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder
            .emit_sdiv_w(destination, WReg::W16, destination);
        Ok(())
    }

    fn emit_remainder_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder.emit_sdiv_w(WReg::W17, WReg::W16, destination);
        self.encoder
            .emit_msub_w(destination, WReg::W17, destination, WReg::W16);
        Ok(())
    }

    fn emit_shift_left_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_w(destination, WReg::W16, destination);
        Ok(())
    }

    fn emit_shift_right_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_asrv_w(destination, WReg::W16, destination);
        Ok(())
    }

    fn emit_i32_shift_count_safety_checks(&mut self, count: WReg) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(count);
        let count_nonnegative = self.emit_cond_branch_placeholder(BranchCondition::Ge);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_nonnegative, "shift non-negative target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, I32_BIT_WIDTH);
        self.encoder.emit_cmp_w(count, WReg::W17);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Lt);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    fn emit_i32_division_safety_checks(
        &mut self,
        dividend: WReg,
        divisor: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(divisor);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, i32::MIN);
        self.encoder.emit_cmp_w(dividend, WReg::W17);
        let dividend_not_min = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, -1);
        self.encoder.emit_cmp_w(divisor, WReg::W17);
        let divisor_not_minus_one = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            dividend_not_min,
            "signed division overflow dividend target",
        )?;
        self.patch_branch_placeholder_to_current(
            divisor_not_minus_one,
            "signed division overflow divisor target",
        )?;

        Ok(())
    }

    fn emit_i32_overflow_check(&mut self, target_description: &str) -> Result<(), Vec<Diagnostic>> {
        let no_overflow = self.emit_cond_branch_placeholder(BranchCondition::Vc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_overflow, target_description)?;
        Ok(())
    }

    fn emit_trap(&mut self) {
        self.encoder.emit_brk(0);
    }

    fn emit_i32_value_to_w(
        &mut self,
        value: &I32Value,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            I32Value::Const(value) => emit_mov_i32_to_w(&mut self.encoder, destination, *value),
            I32Value::Location(location) => {
                let source = self.i32_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_w(destination, source);
                }
            }
        }

        Ok(())
    }

    fn emit_bool_value_to_w(
        &mut self,
        value: &BoolValue,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            BoolValue::Const(value) => {
                emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(*value));
            }
            BoolValue::Location(location) => {
                let source = self.bool_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_w(destination, source);
                }
            }
            BoolValue::Not(_)
            | BoolValue::Logical { .. }
            | BoolValue::I32Comparison { .. }
            | BoolValue::BoolComparison { .. } => {
                let branches_to_false = self.emit_bool_false_branch_placeholders(value)?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 1);
                let branch_to_end = self.emit_branch_placeholder();
                self.patch_branch_placeholders_to_current(
                    branches_to_false,
                    "bool false materialization target",
                )?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 0);
                self.patch_branch_placeholder_to_current(
                    branch_to_end,
                    "bool materialization end target",
                )?;
            }
        }

        Ok(())
    }

    fn i32_location_register(&self, location: I32Location) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            I32Location::Return => Ok(WReg::W0),
            I32Location::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 i32 parameters, got parameter {index}"),
                )]
            }),
            I32Location::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 i32 locals, got local {index}"),
                )]
            }),
        }
    }

    fn bool_location_register(&self, location: BoolLocation) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            BoolLocation::Return => Ok(WReg::W0),
            BoolLocation::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 7 local scalar bindings, got local {index}"),
                )]
            }),
        }
    }

    fn emit_write_static_stderr(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let data_offset = self.read_only_data.len();
        self.read_only_data.extend_from_slice(bytes);

        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        let instruction_offset = self.encoder.position();
        self.encoder.emit_adr_x(XReg::X1, 0);
        self.data_address_patches.push(DataAddressPatch {
            instruction_offset,
            data_offset,
        });
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, bytes.len() as u64);
        emit_darwin_write_syscall(&mut self.encoder);
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
                .patch_adr_x(patch.instruction_offset, XReg::X1, byte_offset as i32);
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
        Self::same_file(&function.name)
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
    data_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionCallPatch {
    instruction_offset: usize,
    function: FunctionSymbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchPatch {
    Unconditional {
        instruction_offset: usize,
    },
    Conditional {
        instruction_offset: usize,
        condition: BranchCondition,
    },
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

fn instruction_list_ends_execution(instructions: &[Instruction]) -> bool {
    match instructions.last() {
        Some(Instruction::Return | Instruction::TailCall { .. }) => true,
        Some(Instruction::If {
            then_instructions,
            else_instructions,
            ..
        }) => {
            !else_instructions.is_empty()
                && instruction_list_ends_execution(then_instructions)
                && instruction_list_ends_execution(else_instructions)
        }
        Some(
            Instruction::WriteStaticStderr(_)
            | Instruction::SetI32 { .. }
            | Instruction::SetBool { .. }
            | Instruction::AddI32 { .. }
            | Instruction::SubtractI32 { .. }
            | Instruction::MultiplyI32 { .. }
            | Instruction::DivideI32 { .. }
            | Instruction::RemainderI32 { .. }
            | Instruction::ShiftLeftI32 { .. }
            | Instruction::ShiftRightI32 { .. }
            | Instruction::CallI32 { .. }
            | Instruction::CallBool { .. },
        )
        | None => false,
    }
}

fn branch_condition_for_true_comparison(operator: I32ComparisonOperator) -> BranchCondition {
    match operator {
        I32ComparisonOperator::Equal => BranchCondition::Eq,
        I32ComparisonOperator::NotEqual => BranchCondition::Ne,
        I32ComparisonOperator::Less => BranchCondition::Lt,
        I32ComparisonOperator::LessEqual => BranchCondition::Le,
        I32ComparisonOperator::Greater => BranchCondition::Gt,
        I32ComparisonOperator::GreaterEqual => BranchCondition::Ge,
    }
}

fn branch_condition_for_false_comparison(operator: I32ComparisonOperator) -> BranchCondition {
    match operator {
        I32ComparisonOperator::Equal => BranchCondition::Ne,
        I32ComparisonOperator::NotEqual => BranchCondition::Eq,
        I32ComparisonOperator::Less => BranchCondition::Ge,
        I32ComparisonOperator::LessEqual => BranchCondition::Gt,
        I32ComparisonOperator::Greater => BranchCondition::Le,
        I32ComparisonOperator::GreaterEqual => BranchCondition::Lt,
    }
}

fn branch_condition_for_true_bool_comparison(operator: BoolComparisonOperator) -> BranchCondition {
    match operator {
        BoolComparisonOperator::Equal => BranchCondition::Eq,
        BoolComparisonOperator::NotEqual => BranchCondition::Ne,
    }
}

fn branch_condition_for_false_bool_comparison(operator: BoolComparisonOperator) -> BranchCondition {
    match operator {
        BoolComparisonOperator::Equal => BranchCondition::Ne,
        BoolComparisonOperator::NotEqual => BranchCondition::Eq,
    }
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

const STDERR_FILENO: u64 = 2;
const ADR_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i64 = (1 << 20) - 1;
const COND_BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const COND_BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 20) - 4;
const BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 27);
const BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 27) - 4;
const DARWIN_WRITE_SYSCALL: u32 = 0x0200_0004;
const DARWIN_EXIT_SYSCALL: u32 = 0x0200_0001;
const DARWIN_SYSCALL_TRAP: u16 = 0x80;
const I32_BIT_WIDTH: i32 = 32;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BoolLocation, BoolValue, CallTarget, Function, I32ComparisonOperator, I32Location,
        I32Value, Type,
    };
    use crate::source::SourceId;

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
            return_type: Type::I32,
            instructions: vec![Instruction::Return],
        };

        assert_eq!(
            FunctionSymbol::from_function(&function),
            FunctionSymbol::SameFile("answer".to_string())
        );
    }

    #[test]
    fn generates_exit_zero_for_return_i32_zero() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
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
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::Return],
        }]);

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0xe0, 0x00, 0x80, 0x52, // movz w0, #7
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[test]
    fn emits_framed_function_prologue_and_return_epilogue() {
        let function = Function {
            name: "main".to_string(),
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
            return_type: Type::I32,
            instructions: vec![tail_call("answer", vec![])],
        };
        let answer = Function {
            name: "answer".to_string(),
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
    fn generates_framed_i32_normal_call_from_hand_built_ir() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(I32Location::Return, "answer", vec![]),
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
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
    fn normal_i32_call_spills_and_reloads_scalar_locals() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
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
                0xe9, 0x03, 0x00, 0xb9, // str w9, [sp, #0]
                0xea, 0x07, 0x00, 0xb9, // str w10, [sp, #4]
                0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
                0xf0, 0x0b, 0x00, 0xb9, // str w16, [sp, #8]
                0xe0, 0x0b, 0x40, 0xb9, // ldr w0, [sp, #8]
                0x0c, 0x00, 0x00, 0x94, // bl add_two
                0xe9, 0x03, 0x40, 0xb9, // ldr w9, [sp, #0]
                0xea, 0x07, 0x40, 0xb9, // ldr w10, [sp, #4]
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

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_i32_normal_call_stages_reordered_arguments() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("wrapper", vec![i32_const(5), i32_const(42)])],
            },
            Function {
                name: "wrapper".to_string(),
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
    fn generated_bool_normal_call_preserves_local_condition() {
        let module = IrModule::new(vec![
            Function {
                name: "main".to_string(),
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
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
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
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
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
                0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
                0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
                0x90, 0x02, 0x80, 0x52, // movz w16, #20
                0xf0, 0x03, 0x00, 0xb9, // str w16, [sp, #0]
                0xd0, 0x02, 0x80, 0x52, // movz w16, #22
                0xf0, 0x07, 0x00, 0xb9, // str w16, [sp, #4]
                0xe0, 0x03, 0x40, 0xb9, // ldr w0, [sp, #0]
                0xe1, 0x07, 0x40, 0xb9, // ldr w1, [sp, #4]
                0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
                0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
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
    fn generates_terminal_if_with_false_condition() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
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

    #[test]
    fn generates_static_stderr_write_with_data_reference() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStaticStderr(b"error\n".to_vec()),
                set_return_i32(1),
                Instruction::Return,
            ],
        }]);

        let code = generate_arm64_darwin_entry(&module, "main").unwrap();

        assert_eq!(code.read_only_data, b"error\n");
        assert_eq!(
            code.text,
            vec![
                0x04, 0x00, 0x00, 0x94, // bl main
                0x30, 0x00, 0x80, 0x52, // movz w16, #1
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x40, 0x00, 0x80, 0xd2, // movz x0, #2
                0xe1, 0x00, 0x00, 0x10, // adr x1, #28
                0xc2, 0x00, 0x80, 0xd2, // movz x2, #6
                0x90, 0x00, 0x80, 0x52, // movz w16, #4
                0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
                0x01, 0x10, 0x00, 0xd4, // svc #0x80
                0x20, 0x00, 0x80, 0x52, // movz w0, #1
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn generated_static_stderr_write_runs() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::WriteStaticStderr(b"failed\n".to_vec()),
                set_return_i32(3),
                Instruction::Return,
            ],
        }]);
        let code = generate_arm64_darwin_entry(&module, "main").unwrap();
        let image = crate::target::macho::write_arm64_macos_executable_with_data(
            &code.text,
            &code.read_only_data,
        );
        let executable = write_temp_executable("codegen-stderr-runs", &image.bytes);

        let output = std::process::Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(executable);

        assert_eq!(output.status.code(), Some(3));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"failed\n");
    }

    #[test]
    fn generates_exit_zero_for_return_void() {
        let module = IrModule::new(vec![Function {
            name: "main".to_string(),
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
            arguments,
        }
    }

    fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
        Instruction::CallI32 {
            destination,
            target: CallTarget::same_file(function),
            arguments,
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
            arguments,
        }
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

    fn contains_instruction(text: &[u8], instruction: [u8; 4]) -> bool {
        text.windows(4).any(|window| window == instruction)
    }
}
