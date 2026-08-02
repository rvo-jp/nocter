use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_process_exit(
        &mut self,
        code: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_value_to_w(code, WReg::W0)?;
        emit_darwin_exit_syscall(&mut self.encoder);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_fallible_process_exit(
        &mut self,
        success_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
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

    pub(in crate::backend::codegen) fn emit_fallible_entry_failure_report(&mut self) {
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

    pub(in crate::backend::codegen) fn emit_stack_str_to_stderr(
        &mut self,
        ptr_offset: u32,
        len_offset: u32,
    ) {
        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, STDERR_FILENO);
        self.encoder.emit_ldr_x_sp(XReg::X1, ptr_offset);
        self.encoder.emit_ldr_x_sp(XReg::X2, len_offset);
        emit_darwin_write_syscall(&mut self.encoder);
    }

    pub(in crate::backend::codegen) fn emit_return_fallible_success(
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

    pub(in crate::backend::codegen) fn emit_fallible_success_payload(
        &mut self,
        success_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_supported_fallible_success_payload_abi(success_type)?;
        match success_type {
            Type::I32 | Type::U8 | Type::Bool => {
                self.encoder.emit_mov_w(WReg::W1, WReg::W0);
            }
            Type::Usize | Type::Borrow { .. } => {
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Str | Type::Slice { .. } => {
                self.encoder.emit_mov_x(XReg::X2, XReg::X1);
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Aggregate { .. } => {}
            Type::DirectAggregate { words, .. } => match words {
                0 => {}
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
            Type::Error | Type::Never | Type::Fallible(_) => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "invalid fallible success payload type for codegen",
                )]);
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_return_fallible_failure(
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

    pub(in crate::backend::codegen) fn emit_failure_payload_to_registers(
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

    pub(in crate::backend::codegen) fn str_value_source_registers(
        &self,
        value: &StrValue,
    ) -> Result<[Option<XReg>; 2], Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(_) => Ok([None, None]),
            StrValue::Location(location) => self.str_location_source_registers(*location),
            StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. } => Ok([None, None]),
        }
    }

    pub(in crate::backend::codegen) fn str_location_source_registers(
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

    pub(in crate::backend::codegen) fn parameter_word_source_register(
        &self,
        index: usize,
    ) -> Option<XReg> {
        if self.current_parameter_spill_offsets.contains_key(&index) {
            return None;
        }
        XReg::argument(index)
    }

    pub(in crate::backend::codegen) fn local_word_source_register(
        &self,
        index: usize,
    ) -> Option<XReg> {
        XReg::local(index)
    }

    pub(in crate::backend::codegen) fn emit_return_optional_none(
        &mut self,
        frame: Option<&FrameLayout>,
    ) {
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        self.emit_return(frame);
    }

    pub(in crate::backend::codegen) fn emit_propagate_failure(
        &mut self,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_return(frame);
        self.patch_branch_placeholder_to_current(success_branch, "fallible success target")
    }

    pub(in crate::backend::codegen) fn emit_trap_on_failure(
        &mut self,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(success_branch, "fallible force success target")
    }

    pub(in crate::backend::codegen) fn emit_check_failure(
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

    pub(in crate::backend::codegen) fn emit_fallible_failure_action(
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
}
