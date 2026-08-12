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

    pub(in crate::backend::codegen) fn emit_return_fallible_failure(
        &mut self,
        code: &StrValue,
        message: &StrValue,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match return_type {
            Type::Fallible(_) => {
                self.emit_failure_payload_to_registers(code, message)?;
                emit_mov_i32_to_w0(&mut self.encoder, 1);
            }
            Type::ComposedOutcome { outer, inner, .. } => match (outer, inner) {
                (crate::outcomes::OutcomeLayer::Fallible, _) => {
                    self.emit_failure_payload_to_registers(code, message)?;
                    emit_mov_i32_to_w0(&mut self.encoder, 1);
                }
                (
                    crate::outcomes::OutcomeLayer::Optional,
                    crate::outcomes::OutcomeLayer::Fallible,
                ) => {
                    self.emit_failure_payload_to_registers_at(code, message, 2)?;
                    emit_mov_i32_to_w(&mut self.encoder, WReg::W1, 1);
                    emit_mov_i32_to_w0(&mut self.encoder, 0);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "E9002",
                        "composed outcome has no fallible return layer",
                    )]);
                }
            },
            Type::Optional(_) => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "fallible failure return requires a fallible outcome layer",
                )]);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "fallible failure requires a fallible return layer",
                )]);
            }
        }
        self.emit_return(frame);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_failure_payload_to_registers(
        &mut self,
        code: &StrValue,
        message: &StrValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_failure_payload_to_registers_at(code, message, 1)
    }

    pub(in crate::backend::codegen) fn emit_failure_payload_to_registers_at(
        &mut self,
        code: &StrValue,
        message: &StrValue,
        start: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        let code_sources = self.str_value_source_registers(code)?;
        let message_sources = self.str_value_source_registers(message)?;
        let code_destinations = [
            XReg::argument(start).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "invalid outcome error payload register",
                )]
            })?,
            XReg::argument(start + 1).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "invalid outcome error payload register",
                )]
            })?,
        ];
        let message_destinations = [
            XReg::argument(start + 2).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "invalid outcome error payload register",
                )]
            })?,
            XReg::argument(start + 3).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "invalid outcome error payload register",
                )]
            })?,
        ];

        let code_clobbers_message = registers_overlap(&code_destinations, &message_sources);
        let message_clobbers_code = registers_overlap(&message_destinations, &code_sources);

        match (code_clobbers_message, message_clobbers_code) {
            (true, true) => {
                let (temporary_ptr, temporary_len) =
                    failure_payload_temporary_pair(&message_sources, &message_destinations)?;
                self.emit_str_value_to_x_pair(code, temporary_ptr, temporary_len)?;
                self.emit_str_value_to_x_pair(
                    message,
                    message_destinations[0],
                    message_destinations[1],
                )?;
                self.emit_x_pair_to_x_pair(
                    temporary_ptr,
                    temporary_len,
                    code_destinations[0],
                    code_destinations[1],
                )?;
            }
            (true, false) => {
                self.emit_str_value_to_x_pair(
                    message,
                    message_destinations[0],
                    message_destinations[1],
                )?;
                self.emit_str_value_to_x_pair(code, code_destinations[0], code_destinations[1])?;
            }
            _ => {
                self.emit_str_value_to_x_pair(code, code_destinations[0], code_destinations[1])?;
                self.emit_str_value_to_x_pair(
                    message,
                    message_destinations[0],
                    message_destinations[1],
                )?;
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
            StrValue::ProcessArg { .. }
            | StrValue::ProcessEnvironmentName { .. }
            | StrValue::ProcessEnvironmentValue { .. }
            | StrValue::SliceIndex { .. } => Ok([None, None]),
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
        failure_mode: &OutcomeFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        match failure_mode {
            OutcomeFailureMode::Propagate => self.emit_return(frame),
            OutcomeFailureMode::PropagateWithCleanup { .. }
            | OutcomeFailureMode::Handle { .. }
            | OutcomeFailureMode::Recover { .. } => {
                let Some(frame) = frame else {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "fallible failure handler emission requires a stack frame",
                    )]);
                };
                self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
            }
            OutcomeFailureMode::Trap => self.emit_trap(),
            OutcomeFailureMode::Catch { .. } => {
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
        failure_mode: &OutcomeFailureMode,
        frame: &FrameLayout,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match failure_mode {
            OutcomeFailureMode::Propagate => {
                self.emit_return(Some(frame));
                Ok(())
            }
            OutcomeFailureMode::PropagateWithCleanup {
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
            OutcomeFailureMode::Trap => {
                self.emit_trap();
                Ok(())
            }
            OutcomeFailureMode::Handle { instructions } => {
                self.emit_scalar_reloads(frame)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            OutcomeFailureMode::Recover { instructions } => {
                self.emit_scalar_reloads(frame)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            OutcomeFailureMode::Catch {
                code,
                message,
                instructions,
                ..
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
