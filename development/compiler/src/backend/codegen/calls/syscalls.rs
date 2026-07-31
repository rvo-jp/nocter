use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_darwin_syscall(
        &mut self,
        destination: AggregateLocation,
        arity: u8,
        number: &UsizeValue,
        arguments: &[UsizeValue],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "macOS syscall emission requires a stack frame",
            )]);
        };
        let arity = usize::from(arity);
        if arguments.len() != arity || arity > 6 {
            return Err(vec![Diagnostic::error(
                "E9003",
                "macOS syscall emission requires 0 to 6 syscall arguments",
            )]);
        }

        self.emit_scalar_spills(frame)?;
        self.emit_staged_syscall_words(number, arguments, frame)?;
        self.encoder.emit_svc(DARWIN_SYSCALL_TRAP);
        self.emit_darwin_syscall_result_to_location(destination, frame)?;
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_staged_syscall_words(
        &mut self,
        number: &UsizeValue,
        arguments: &[UsizeValue],
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_syscall_word_to_staging(0, number, frame)?;
        for (index, argument) in arguments.iter().enumerate() {
            let abi_word_index = next_abi_word_index(index, "macOS syscall argument index")?;
            self.emit_syscall_word_to_staging(abi_word_index, argument, frame)?;
        }

        let number_slot = staging_slot(frame, 0)?;
        self.encoder.emit_ldr_x_sp(XReg::X16, number_slot.offset());
        for index in 0..arguments.len() {
            let Some(register) = XReg::argument(index) else {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    format!("macOS syscall argument {index} has no ARM64 argument register"),
                )]);
            };
            let abi_word_index = next_abi_word_index(index, "macOS syscall argument index")?;
            let slot = staging_slot(frame, abi_word_index)?;
            self.encoder.emit_ldr_x_sp(register, slot.offset());
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_syscall_word_to_staging(
        &mut self,
        index: usize,
        value: &UsizeValue,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let slot = staging_slot(frame, index)?;
        self.emit_usize_value_to_x(value, XReg::X16)?;
        self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_darwin_syscall_result_to_location(
        &mut self,
        destination: AggregateLocation,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.encoder.emit_mov_w(WReg::W17, WReg::W0);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X16, 0);
        let store_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholder_to_current(success_branch, "syscall success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X0);
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, 0);

        self.patch_branch_placeholder_to_current(store_branch, "syscall result store target")?;
        self.emit_syscall_result_words_to_location(destination, frame)
    }

    pub(in crate::backend::codegen) fn emit_syscall_result_words_to_location(
        &mut self,
        destination: AggregateLocation,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::DirectReturn => {
                self.encoder.emit_mov_x(XReg::X0, XReg::X16);
                self.encoder.emit_mov_x(XReg::X1, XReg::X17);
                Ok(())
            }
            AggregateLocation::Slot(slot_index) => {
                let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("syscall result destination slot {slot_index} is not reserved"),
                    )]
                })?;
                let errno_offset = slot
                    .offset()
                    .checked_add(ABI_WORD_SIZE as u32)
                    .ok_or_else(syscall_result_store_offset_diagnostic)?;
                self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                self.encoder.emit_str_w_sp(WReg::W17, errno_offset);
                Ok(())
            }
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(Some(frame));
                self.encoder.emit_str_x_imm(XReg::X16, XReg::X8, 0);
                self.encoder
                    .emit_str_w_imm(WReg::W17, XReg::X8, ABI_WORD_SIZE as u32);
                Ok(())
            }
            AggregateLocation::Parameter(_) | AggregateLocation::DirectParameter { .. } => {
                Err(vec![Diagnostic::error(
                    "E9005",
                    "syscall result cannot be stored into parameter locations",
                )])
            }
        }
    }
}
