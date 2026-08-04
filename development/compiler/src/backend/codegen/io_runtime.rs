use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_write_static_stderr(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        self.emit_static_data_address(XReg::X1, bytes);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, bytes.len() as u64);
        emit_darwin_write_syscall(&mut self.encoder);
    }

    pub(in crate::backend::codegen) fn emit_write_str(
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

    pub(in crate::backend::codegen) fn emit_write_slice(
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

    pub(in crate::backend::codegen) fn emit_write_all_syscall_loop(
        &mut self,
    ) -> Result<(), Vec<Diagnostic>> {
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

    pub(in crate::backend::codegen) fn emit_read_slice(
        &mut self,
        destination: UsizeLocation,
        fd: &I32Value,
        buffer: &SliceValue,
        failure_mode: &OutcomeFailureMode,
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

    pub(in crate::backend::codegen) fn emit_open_read(
        &mut self,
        destination: I32Location,
        path: &UsizeValue,
        failure_mode: &OutcomeFailureMode,
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

    pub(in crate::backend::codegen) fn emit_close_fd(
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

    pub(in crate::backend::codegen) fn emit_set_usize_from_borrow(
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
}
