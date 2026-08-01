use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_add_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.encoder.emit_add_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_u8_range_check(WReg::W16, "u8 addition in-range target")?;
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.encoder.emit_add_w(destination, WReg::W16, destination);
        self.emit_u8_range_check(destination, "u8 addition in-range target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_subtract_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.encoder.emit_sub_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_u8_range_check(WReg::W16, "u8 subtraction in-range target")?;
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.encoder.emit_sub_w(destination, WReg::W16, destination);
        self.emit_u8_range_check(destination, "u8 subtraction in-range target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_multiply_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.encoder.emit_mul_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_u8_range_check(WReg::W16, "u8 multiplication in-range target")?;
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.encoder.emit_mul_w(destination, WReg::W16, destination);
        self.emit_u8_range_check(destination, "u8 multiplication in-range target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_divide_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.emit_u8_division_safety_checks(WReg::W17)?;
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.encoder.emit_udiv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.emit_u8_division_safety_checks(destination)?;
        self.encoder
            .emit_udiv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_remainder_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.emit_u8_division_safety_checks(WReg::W17)?;
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.encoder.emit_udiv_w(WReg::W8, WReg::W16, WReg::W17);
            self.encoder
                .emit_msub_w(WReg::W16, WReg::W8, WReg::W17, WReg::W16);
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.emit_u8_division_safety_checks(destination)?;
        self.encoder.emit_udiv_w(WReg::W17, WReg::W16, destination);
        self.encoder
            .emit_msub_w(destination, WReg::W17, destination, WReg::W16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_left_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.emit_u8_shift_count_safety_checks(WReg::W17)?;
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.encoder.emit_lslv_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_u8_range_check(WReg::W16, "u8 shift-left in-range target")?;
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.emit_u8_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_w(destination, WReg::W16, destination);
        self.emit_u8_range_check(destination, "u8 shift-left in-range target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_right_u8(
        &mut self,
        destination: U8Location,
        left: &U8Value,
        right: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.u8_register_destination(destination)? else {
            self.emit_u8_value_to_w(right, WReg::W17)?;
            self.emit_u8_shift_count_safety_checks(WReg::W17)?;
            self.emit_u8_value_to_w(left, WReg::W16)?;
            self.encoder.emit_lsrv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_u8_location(WReg::W16, destination);
        };
        self.emit_u8_value_to_w(left, WReg::W16)?;
        self.emit_u8_value_to_w(right, destination)?;
        self.emit_u8_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lsrv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_add_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_adds_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_i32_overflow_check("i32 addition non-overflow target")?;
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_adds_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 addition non-overflow target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_subtract_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_subs_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_i32_overflow_check("i32 subtraction non-overflow target")?;
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_subs_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 subtraction non-overflow target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_multiply_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_smull_x(XReg::X17, WReg::W16, WReg::W17);
            self.encoder.emit_sxtw_x_w(XReg::X16, WReg::W17);
            self.encoder.emit_cmp_x(XReg::X17, XReg::X16);
            let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
            self.emit_trap();
            self.patch_branch_placeholder_to_current(
                exact_fit,
                "i32 multiplication exact-fit target",
            )?;
            return self.emit_w_to_i32_location(WReg::W17, destination);
        };
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

    pub(in crate::backend::codegen) fn emit_divide_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_division_safety_checks_for_values(left, right)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_sdiv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder
            .emit_sdiv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_remainder_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_division_safety_checks_for_values(left, right)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_sdiv_w(WReg::W8, WReg::W16, WReg::W17);
            self.encoder
                .emit_msub_w(WReg::W16, WReg::W8, WReg::W17, WReg::W16);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder.emit_sdiv_w(WReg::W17, WReg::W16, destination);
        self.encoder
            .emit_msub_w(destination, WReg::W17, destination, WReg::W16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_left_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.emit_i32_shift_count_safety_checks_with_scratch(WReg::W17, WReg::W16)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.encoder.emit_lslv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_right_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.emit_i32_shift_count_safety_checks_with_scratch(WReg::W17, WReg::W16)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.encoder.emit_asrv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_asrv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_add_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.encoder.emit_adds_x(XReg::X16, XReg::X16, XReg::X17);
            self.emit_usize_no_carry_check("usize addition non-overflow target")?;
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder
            .emit_adds_x(destination, XReg::X16, destination);
        self.emit_usize_no_carry_check("usize addition non-overflow target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_subtract_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.encoder.emit_subs_x(XReg::X16, XReg::X16, XReg::X17);
            self.emit_usize_no_borrow_check("usize subtraction non-underflow target")?;
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder
            .emit_subs_x(destination, XReg::X16, destination);
        self.emit_usize_no_borrow_check("usize subtraction non-underflow target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_multiply_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X8)?;
            self.encoder.emit_umulh_x(XReg::X17, XReg::X16, XReg::X8);
            self.encoder.emit_cmp_x_zero(XReg::X17);
            let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
            self.emit_trap();
            self.patch_branch_placeholder_to_current(
                exact_fit,
                "usize multiplication exact-fit target",
            )?;
            self.encoder.emit_mul_x(XReg::X16, XReg::X16, XReg::X8);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder.emit_umulh_x(XReg::X17, XReg::X16, destination);
        self.encoder.emit_cmp_x_zero(XReg::X17);
        let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            exact_fit,
            "usize multiplication exact-fit target",
        )?;
        self.encoder.emit_mul_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_divide_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_division_safety_checks(XReg::X17)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_udiv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_division_safety_checks(destination)?;
        self.encoder
            .emit_udiv_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_remainder_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_division_safety_checks(XReg::X17)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_udiv_x(XReg::X8, XReg::X16, XReg::X17);
            self.encoder
                .emit_msub_x(XReg::X16, XReg::X8, XReg::X17, XReg::X16);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_division_safety_checks(destination)?;
        self.encoder.emit_udiv_x(XReg::X17, XReg::X16, destination);
        self.encoder
            .emit_msub_x(destination, XReg::X17, destination, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_left_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_shift_count_safety_checks_with_scratch(XReg::X17, XReg::X16)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_lslv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_shift_right_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_shift_count_safety_checks_with_scratch(XReg::X17, XReg::X16)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_lsrv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lsrv_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_u8_range_check(
        &mut self,
        value: WReg,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, i32::from(u8::MAX));
        self.encoder.emit_cmp_w(value, WReg::W17);
        let in_range = self.emit_cond_branch_placeholder(BranchCondition::Ls);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(in_range, target_description)?;
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_u8_division_safety_checks(
        &mut self,
        divisor: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(divisor);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_u8_shift_count_safety_checks(
        &mut self,
        count: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let scratch = if count == WReg::W17 {
            WReg::W16
        } else {
            WReg::W17
        };
        emit_mov_i32_to_w(&mut self.encoder, scratch, 8);
        self.encoder.emit_cmp_w(count, scratch);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "u8 shift count in-range target")?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_i32_shift_count_safety_checks(
        &mut self,
        count: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_shift_count_safety_checks_with_scratch(count, WReg::W17)
    }

    pub(in crate::backend::codegen::values) fn emit_i32_shift_count_safety_checks_with_scratch(
        &mut self,
        count: WReg,
        scratch: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(count);
        let count_nonnegative = self.emit_cond_branch_placeholder(BranchCondition::Ge);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_nonnegative, "shift non-negative target")?;

        emit_mov_i32_to_w(&mut self.encoder, scratch, I32_BIT_WIDTH);
        self.encoder.emit_cmp_w(count, scratch);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Lt);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_usize_shift_count_safety_checks(
        &mut self,
        count: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_shift_count_safety_checks_with_scratch(count, XReg::X17)
    }

    pub(in crate::backend::codegen::values) fn emit_usize_shift_count_safety_checks_with_scratch(
        &mut self,
        count: XReg,
        scratch: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        emit_mov_u64_to_x(&mut self.encoder, scratch, USIZE_BIT_WIDTH);
        self.encoder.emit_cmp_x(count, scratch);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_usize_division_safety_checks(
        &mut self,
        divisor: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(divisor);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_i32_division_safety_checks(
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

    pub(in crate::backend::codegen::values) fn emit_i32_division_safety_checks_for_values(
        &mut self,
        dividend: &I32Value,
        divisor: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_value_to_w(divisor, WReg::W17)?;
        self.encoder.emit_cmp_w_zero(WReg::W17);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W16, -1);
        self.encoder.emit_cmp_w(WReg::W17, WReg::W16);
        let divisor_not_minus_one = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_i32_value_to_w(dividend, WReg::W16)?;
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, i32::MIN);
        self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
        let dividend_not_min = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            divisor_not_minus_one,
            "signed division overflow divisor target",
        )?;
        self.patch_branch_placeholder_to_current(
            dividend_not_min,
            "signed division overflow dividend target",
        )?;

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_usize_no_carry_check(
        &mut self,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let no_carry = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_carry, target_description)?;
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_usize_no_borrow_check(
        &mut self,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let no_borrow = self.emit_cond_branch_placeholder(BranchCondition::Cs);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_borrow, target_description)?;
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_i32_overflow_check(
        &mut self,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let no_overflow = self.emit_cond_branch_placeholder(BranchCondition::Vc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_overflow, target_description)?;
        Ok(())
    }
}
