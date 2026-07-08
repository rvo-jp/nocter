use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, BoolLogicalOperator, BoolValue, Function, I32ComparisonOperator, I32Location,
    I32Value, Instruction, IrModule, Type,
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
    function_offsets: HashMap<String, usize>,
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
        self.emit_call(&entry.name);
        if matches!(entry.return_type.success_type(), Type::Void) {
            emit_mov_i32_to_w0(&mut self.encoder, 0);
        }
        emit_darwin_exit_syscall(&mut self.encoder);
    }

    fn emit_function(&mut self, function: &Function) -> Result<(), Vec<Diagnostic>> {
        self.function_offsets
            .insert(function.name.clone(), self.encoder.position());

        for instruction in &function.instructions {
            self.emit_instruction(instruction)?;
        }

        Ok(())
    }

    fn emit_instruction(&mut self, instruction: &Instruction) -> Result<(), Vec<Diagnostic>> {
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
            Instruction::TailCall {
                function,
                arguments,
            } => {
                self.emit_tail_call(function, arguments)?;
            }
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => {
                self.emit_if(condition, then_instructions, else_instructions)?;
            }
            Instruction::Return => {
                self.encoder.emit_ret();
            }
        }

        Ok(())
    }

    fn emit_if(
        &mut self,
        condition: &BoolValue,
        then_instructions: &[Instruction],
        else_instructions: &[Instruction],
    ) -> Result<(), Vec<Diagnostic>> {
        let branches_to_else = self.emit_bool_false_branch_placeholders(condition)?;

        for instruction in then_instructions {
            self.emit_instruction(instruction)?;
        }

        let branch_to_end =
            if else_instructions.is_empty() || instruction_list_ends_execution(then_instructions) {
                None
            } else {
                Some(self.emit_branch_placeholder())
            };

        self.patch_branch_placeholders_to_current(branches_to_else, "if branch target")?;

        for instruction in else_instructions {
            self.emit_instruction(instruction)?;
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

    fn emit_call(&mut self, function: &str) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_bl(0);
        self.call_patches.push(FunctionCallPatch {
            instruction_offset,
            function: function.to_string(),
        });
    }

    fn emit_tail_call(
        &mut self,
        function: &str,
        arguments: &[I32Value],
    ) -> Result<(), Vec<Diagnostic>> {
        for (index, argument) in arguments.iter().enumerate() {
            let Some(register) = WReg::argument(index) else {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 i32 arguments, got argument {index}"),
                )]);
            };
            self.emit_i32_value_to_w(argument, register)?;
        }

        let instruction_offset = self.encoder.position();
        self.encoder.emit_b(0);
        self.tail_call_patches.push(FunctionCallPatch {
            instruction_offset,
            function: function.to_string(),
        });

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
        self.encoder.emit_add_w(destination, WReg::W16, destination);
        Ok(())
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
            BoolValue::Not(_) | BoolValue::Logical { .. } | BoolValue::I32Comparison { .. } => {
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
                format!("codegen could not resolve function `{}`", patch.function),
            )]);
        };

        let byte_offset = *target_offset as i64 - patch.instruction_offset as i64;
        if !(BRANCH_MIN_BYTE_OFFSET..=BRANCH_MAX_BYTE_OFFSET).contains(&byte_offset) {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "function `{}` is too far from call site for ARM64 `{instruction}`",
                    patch.function
                ),
            )]);
        }

        Ok(byte_offset)
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
    function: String,
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
            | Instruction::AddI32 { .. },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BoolLocation, BoolValue, Function, I32ComparisonOperator, I32Location, I32Value, Type,
    };

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
                0x80, 0x02, 0x80, 0x52, // movz w0, #20
                0xc1, 0x02, 0x80, 0x52, // movz w1, #22
                0x01, 0x00, 0x00, 0x14, // b add
                0xf0, 0x03, 0x00, 0x2a, // mov w16, w0
                0xe0, 0x03, 0x01, 0x2a, // mov w0, w1
                0x00, 0x02, 0x00, 0x0b, // add w0, w16, w0
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
                0x0a, 0x02, 0x0a, 0x0b, // add w10, w16, w10
                0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
                0xc0, 0x03, 0x5f, 0xd6, // ret
            ]
        );
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

    fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
        Instruction::TailCall {
            function: function.to_string(),
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
}
