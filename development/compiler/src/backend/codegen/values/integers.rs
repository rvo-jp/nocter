use super::*;
use crate::integer::IntegerType;
use crate::ir::IntegerBinaryOperator;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_integer_binary(
        &mut self,
        kind: IntegerType,
        operator: IntegerBinaryOperator,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, XReg::X17)?;

        match operator {
            IntegerBinaryOperator::Add => {
                self.encoder.emit_adds_x(XReg::X8, XReg::X16, XReg::X17);
                if kind.bit_width() == 64 {
                    self.emit_integer_add_overflow_check(kind)?;
                } else {
                    self.emit_integer_range_check(kind, XReg::X8)?;
                }
            }
            IntegerBinaryOperator::Subtract => {
                self.encoder.emit_subs_x(XReg::X8, XReg::X16, XReg::X17);
                if kind.bit_width() == 64 {
                    self.emit_integer_subtract_overflow_check(kind)?;
                } else {
                    self.emit_integer_range_check(kind, XReg::X8)?;
                }
            }
            IntegerBinaryOperator::Multiply => {
                if kind.bit_width() == 64 {
                    if kind.is_signed() {
                        self.encoder.emit_smulh_x(XReg::X8, XReg::X16, XReg::X17);
                        self.encoder.emit_mul_x(XReg::X16, XReg::X16, XReg::X17);
                        self.encoder.emit_asr_x_imm(XReg::X17, XReg::X16, 63);
                        self.encoder.emit_cmp_x(XReg::X8, XReg::X17);
                        self.emit_trap_unless(
                            BranchCondition::Eq,
                            "signed integer multiplication exact-fit target",
                        )?;
                        self.encoder.emit_mov_x(XReg::X8, XReg::X16);
                    } else {
                        self.encoder.emit_umulh_x(XReg::X8, XReg::X16, XReg::X17);
                        self.encoder.emit_cmp_x_zero(XReg::X8);
                        self.emit_trap_unless(
                            BranchCondition::Eq,
                            "unsigned integer multiplication exact-fit target",
                        )?;
                        self.encoder.emit_mul_x(XReg::X8, XReg::X16, XReg::X17);
                    }
                } else {
                    self.encoder.emit_mul_x(XReg::X8, XReg::X16, XReg::X17);
                    self.emit_integer_range_check(kind, XReg::X8)?;
                }
            }
            IntegerBinaryOperator::Divide | IntegerBinaryOperator::Remainder => {
                self.emit_integer_division_safety_checks(kind, XReg::X16, XReg::X17)?;
                if kind.is_signed() {
                    self.encoder.emit_sdiv_x(XReg::X8, XReg::X16, XReg::X17);
                } else {
                    self.encoder.emit_udiv_x(XReg::X8, XReg::X16, XReg::X17);
                }
                if operator == IntegerBinaryOperator::Remainder {
                    self.encoder
                        .emit_msub_x(XReg::X8, XReg::X8, XReg::X17, XReg::X16);
                }
                if kind.bit_width() < 64 {
                    self.emit_integer_range_check(kind, XReg::X8)?;
                }
            }
            IntegerBinaryOperator::ShiftLeft | IntegerBinaryOperator::ShiftRight => {
                self.emit_integer_shift_count_check(kind, XReg::X17)?;
                match operator {
                    IntegerBinaryOperator::ShiftLeft => {
                        self.encoder.emit_lslv_x(XReg::X8, XReg::X16, XReg::X17);
                        if kind.bit_width() < 64 {
                            self.emit_integer_range_check(kind, XReg::X8)?;
                        }
                    }
                    IntegerBinaryOperator::ShiftRight if kind.is_signed() => {
                        self.encoder.emit_asrv_x(XReg::X8, XReg::X16, XReg::X17);
                    }
                    IntegerBinaryOperator::ShiftRight => {
                        self.encoder.emit_lsrv_x(XReg::X8, XReg::X16, XReg::X17);
                    }
                    _ => unreachable!(),
                }
            }
        }

        self.emit_x_to_usize_location(XReg::X8, destination)
    }

    fn emit_integer_add_overflow_check(
        &mut self,
        kind: IntegerType,
    ) -> Result<(), Vec<Diagnostic>> {
        let condition = if kind.is_signed() {
            BranchCondition::Vc
        } else {
            BranchCondition::Cc
        };
        self.emit_trap_unless(condition, "integer addition non-overflow target")
    }

    fn emit_integer_subtract_overflow_check(
        &mut self,
        kind: IntegerType,
    ) -> Result<(), Vec<Diagnostic>> {
        let condition = if kind.is_signed() {
            BranchCondition::Vc
        } else {
            BranchCondition::Cs
        };
        self.emit_trap_unless(condition, "integer subtraction non-overflow target")
    }

    fn emit_integer_range_check(
        &mut self,
        kind: IntegerType,
        value: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        debug_assert!(kind.bit_width() < 64);
        if kind.is_signed() {
            let max = (1_u64 << (kind.bit_width() - 1)) - 1;
            let min = (!max).wrapping_add(1);
            emit_mov_u64_to_x(&mut self.encoder, XReg::X16, min);
            self.encoder.emit_cmp_x(value, XReg::X16);
            self.emit_trap_unless(BranchCondition::Ge, "signed integer minimum target")?;
            emit_mov_u64_to_x(&mut self.encoder, XReg::X16, max);
            self.encoder.emit_cmp_x(value, XReg::X16);
            self.emit_trap_unless(BranchCondition::Le, "signed integer maximum target")?;
        } else {
            emit_mov_u64_to_x(&mut self.encoder, XReg::X16, kind.mask());
            self.encoder.emit_cmp_x(value, XReg::X16);
            self.emit_trap_unless(BranchCondition::Ls, "unsigned integer maximum target")?;
        }
        Ok(())
    }

    fn emit_integer_division_safety_checks(
        &mut self,
        kind: IntegerType,
        dividend: XReg,
        divisor: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(divisor);
        self.emit_trap_unless(BranchCondition::Ne, "integer division non-zero target")?;
        if !kind.is_signed() {
            return Ok(());
        }

        let minimum = if kind.bit_width() == 64 {
            i64::MIN as u64
        } else {
            kind.canonical_word(1_u64 << (kind.bit_width() - 1))
        };
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, minimum);
        self.encoder.emit_cmp_x(dividend, XReg::X8);
        let dividend_not_min = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, u64::MAX);
        self.encoder.emit_cmp_x(divisor, XReg::X8);
        let divisor_not_minus_one = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            dividend_not_min,
            "signed division non-minimum target",
        )?;
        self.patch_branch_placeholder_to_current(
            divisor_not_minus_one,
            "signed division non-minus-one target",
        )?;
        Ok(())
    }

    fn emit_integer_shift_count_check(
        &mut self,
        kind: IntegerType,
        count: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        if kind.is_signed() {
            self.encoder.emit_cmp_x_zero(count);
            self.emit_trap_unless(BranchCondition::Ge, "integer shift non-negative target")?;
        }
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, u64::from(kind.bit_width()));
        self.encoder.emit_cmp_x(count, XReg::X8);
        self.emit_trap_unless(BranchCondition::Cc, "integer shift count in-range target")
    }

    fn emit_trap_unless(
        &mut self,
        condition: BranchCondition,
        target: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let valid = self.emit_cond_branch_placeholder(condition);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(valid, target)
    }
}
