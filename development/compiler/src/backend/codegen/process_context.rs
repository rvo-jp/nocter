use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_process_environment_count_to_x(
        &mut self,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_environment_pointer_count(XReg::X8)?;
        if destination != XReg::X8 {
            self.encoder.emit_mov_x(destination, XReg::X8);
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_process_environment_name_to_x_pair(
        &mut self,
        index: &UsizeValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_process_environment_entry_pointer(index)?;

        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);
        let loop_start = self.encoder.position();
        self.encoder.emit_ldrb_w_reg(WReg::W17, XReg::X16, XReg::X8);
        self.encoder.emit_cmp_w_zero(WReg::W17);
        let done_at_nul = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_cmp_w_imm(WReg::W17, u32::from(b'='));
        let done_at_separator = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        let repeat = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            repeat,
            loop_start,
            "process environment name scan loop",
        )?;
        self.patch_branch_placeholder_to_current(
            done_at_nul,
            "process environment name NUL target",
        )?;
        self.patch_branch_placeholder_to_current(
            done_at_separator,
            "process environment name separator target",
        )?;

        self.emit_x_pair_to_x_pair(XReg::X16, XReg::X8, ptr_destination, len_destination)
    }

    pub(in crate::backend::codegen) fn emit_process_environment_value_to_x_pair(
        &mut self,
        index: &UsizeValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_process_environment_entry_pointer(index)?;

        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);
        let separator_loop = self.encoder.position();
        self.encoder.emit_ldrb_w_reg(WReg::W17, XReg::X16, XReg::X8);
        self.encoder.emit_cmp_w_zero(WReg::W17);
        let no_separator = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_cmp_w_imm(WReg::W17, u32::from(b'='));
        let separator = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        let repeat_separator = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            repeat_separator,
            separator_loop,
            "process environment separator scan loop",
        )?;

        self.patch_branch_placeholder_to_current(
            no_separator,
            "process environment entry without separator",
        )?;
        self.encoder.emit_adds_x(XReg::X16, XReg::X16, XReg::X8);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);
        let no_separator_finish = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(
            separator,
            "process environment value separator target",
        )?;
        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        self.encoder.emit_adds_x(XReg::X16, XReg::X16, XReg::X8);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);
        let value_loop = self.encoder.position();
        self.encoder.emit_ldrb_w_reg(WReg::W17, XReg::X16, XReg::X8);
        self.encoder.emit_cmp_w_zero(WReg::W17);
        let value_done = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        let repeat_value = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            repeat_value,
            value_loop,
            "process environment value scan loop",
        )?;
        self.patch_branch_placeholder_to_current(
            value_done,
            "process environment value scan done",
        )?;
        self.patch_branch_placeholder_to_current(
            no_separator_finish,
            "process environment empty value target",
        )?;

        self.emit_x_pair_to_x_pair(XReg::X16, XReg::X8, ptr_destination, len_destination)
    }

    fn emit_environment_pointer_count(&mut self, destination: XReg) -> Result<(), Vec<Diagnostic>> {
        emit_mov_u64_to_x(&mut self.encoder, destination, 0);
        let loop_start = self.encoder.position();
        self.encoder.emit_lsl_x_imm(XReg::X17, destination, 3);
        self.encoder.emit_adds_x(XReg::X17, XReg::X22, XReg::X17);
        self.encoder.emit_ldr_x_imm(XReg::X17, XReg::X17, 0);
        self.encoder.emit_cmp_x_zero(XReg::X17);
        let done = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_add_x_imm(destination, destination, 1);
        let repeat = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            repeat,
            loop_start,
            "process environment pointer scan loop",
        )?;
        self.patch_branch_placeholder_to_current(done, "process environment pointer scan done")
    }

    fn emit_process_environment_entry_pointer(
        &mut self,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_environment_pointer_count(XReg::X8)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 3);
        self.encoder.emit_adds_x(XReg::X16, XReg::X22, XReg::X16);
        self.encoder.emit_ldr_x_imm(XReg::X16, XReg::X16, 0);
        Ok(())
    }
}
