use super::{BRANCH_MAX_BYTE_OFFSET, BRANCH_MIN_BYTE_OFFSET, EntryEmitter};
use crate::backend::frame::FrameLayout;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLogicalOperator, BoolValue, I32ComparisonOperator, Instruction,
};
use crate::target::arm64::{BranchCondition, WReg, XReg};

impl EntryEmitter {
    pub(super) fn emit_if(
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

    pub(super) fn emit_bool_false_branch_placeholders(
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
            BoolValue::UsizeComparison {
                operator,
                left,
                right,
            } => {
                self.emit_usize_value_to_x(left, XReg::X16)?;
                self.emit_usize_value_to_x(right, XReg::X17)?;
                self.encoder.emit_cmp_x(XReg::X16, XReg::X17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_false_unsigned_comparison(*operator),
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

    pub(super) fn emit_bool_true_branch_placeholders(
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
            BoolValue::UsizeComparison {
                operator,
                left,
                right,
            } => {
                self.emit_usize_value_to_x(left, XReg::X16)?;
                self.emit_usize_value_to_x(right, XReg::X17)?;
                self.encoder.emit_cmp_x(XReg::X16, XReg::X17);
                Ok(vec![self.emit_cond_branch_placeholder(
                    branch_condition_for_true_unsigned_comparison(*operator),
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

    pub(super) fn emit_branch_placeholder(&mut self) -> BranchPatch {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_b(0);
        BranchPatch::Unconditional { instruction_offset }
    }

    pub(super) fn emit_cond_branch_placeholder(
        &mut self,
        condition: BranchCondition,
    ) -> BranchPatch {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_b_cond(condition, 0);
        BranchPatch::Conditional {
            instruction_offset,
            condition,
        }
    }

    pub(super) fn patch_branch_placeholders_to_current(
        &mut self,
        branches: Vec<BranchPatch>,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        for branch in branches {
            self.patch_branch_placeholder_to_current(branch, target_description)?;
        }

        Ok(())
    }

    pub(super) fn patch_branch_placeholder_to_current(
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BranchPatch {
    Unconditional {
        instruction_offset: usize,
    },
    Conditional {
        instruction_offset: usize,
        condition: BranchCondition,
    },
}

fn instruction_list_ends_execution(instructions: &[Instruction]) -> bool {
    match instructions.last() {
        Some(Instruction::Return | Instruction::TailCall { .. } | Instruction::Trap) => true,
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
            | Instruction::SetUsize { .. }
            | Instruction::SetBool { .. }
            | Instruction::SetStr { .. }
            | Instruction::SetSlice { .. }
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
            | Instruction::CallUsize { .. }
            | Instruction::CallBool { .. }
            | Instruction::CallStr { .. }
            | Instruction::CallSlice { .. },
        )
        | None => false,
    }
}

pub(super) fn branch_condition_for_true_comparison(
    operator: I32ComparisonOperator,
) -> BranchCondition {
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

fn branch_condition_for_true_unsigned_comparison(
    operator: I32ComparisonOperator,
) -> BranchCondition {
    match operator {
        I32ComparisonOperator::Equal => BranchCondition::Eq,
        I32ComparisonOperator::NotEqual => BranchCondition::Ne,
        I32ComparisonOperator::Less => BranchCondition::Cc,
        I32ComparisonOperator::LessEqual => BranchCondition::Ls,
        I32ComparisonOperator::Greater => BranchCondition::Hi,
        I32ComparisonOperator::GreaterEqual => BranchCondition::Cs,
    }
}

fn branch_condition_for_false_unsigned_comparison(
    operator: I32ComparisonOperator,
) -> BranchCondition {
    match operator {
        I32ComparisonOperator::Equal => BranchCondition::Ne,
        I32ComparisonOperator::NotEqual => BranchCondition::Eq,
        I32ComparisonOperator::Less => BranchCondition::Cs,
        I32ComparisonOperator::LessEqual => BranchCondition::Hi,
        I32ComparisonOperator::Greater => BranchCondition::Ls,
        I32ComparisonOperator::GreaterEqual => BranchCondition::Cc,
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

const COND_BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 20);
const COND_BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 20) - 4;
