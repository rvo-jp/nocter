use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_call(&mut self, function: FunctionSymbol) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_bl(0);
        self.call_patches.push(FunctionCallPatch {
            instruction_offset,
            function,
        });
    }

    pub(in crate::backend::codegen) fn emit_tail_call(
        &mut self,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !arguments.is_empty() {
            if tail_call_has_borrow_argument(arguments) {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    "tail call emission does not support borrow arguments",
                )]);
            }
            if call_argument_abi_word_count(arguments) > ARGUMENT_REGISTER_COUNT {
                return Err(vec![Diagnostic::error(
                    "E9003",
                    "tail call emission does not support stack-passed arguments",
                )]);
            }
            let Some(frame) = frame else {
                return Err(vec![Diagnostic::error(
                    "E9005",
                    "tail call argument staging requires a stack frame",
                )]);
            };
            let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;
            debug_assert_eq!(outgoing_stack.area_size, 0);
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

    pub(in crate::backend::codegen) fn emit_call_void(
        &mut self,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal void call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_void(
        &mut self,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible void call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_i32(
        &mut self,
        destination: I32Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal i32 call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_i32_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_i32(
        &mut self,
        destination: I32Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible i32 call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_i32_location(WReg::W16, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_usize(
        &mut self,
        destination: UsizeLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal usize call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_usize_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_usize(
        &mut self,
        destination: UsizeLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible usize call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_to_usize_location(XReg::X16, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_u8(
        &mut self,
        destination: U8Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal u8 call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_u8_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_u8(
        &mut self,
        destination: U8Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible u8 call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_u8_location(WReg::W16, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_bool(
        &mut self,
        destination: BoolLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal bool call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_bool_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_bool(
        &mut self,
        destination: BoolLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible bool call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_bool_location(WReg::W16, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_str(
        &mut self,
        destination: StrLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal str call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_str_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_str(
        &mut self,
        destination: StrLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible str call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.encoder.emit_mov_x(XReg::X17, XReg::X2);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_slice(
        &mut self,
        destination: SliceLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal slice call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        self.emit_call_result_to_slice_location(destination)
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_slice(
        &mut self,
        destination: SliceLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible slice call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.encoder.emit_mov_x(XReg::X17, XReg::X2);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)?;
        self.patch_recover_done_branch(recover_done_branch)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_recover_done_branch_if_needed(
        &mut self,
        failure_mode: &OutcomeFailureMode,
    ) -> Option<BranchPatch> {
        matches!(failure_mode, OutcomeFailureMode::Recover { .. })
            .then(|| self.emit_branch_placeholder())
    }

    pub(in crate::backend::codegen) fn patch_recover_done_branch(
        &mut self,
        branch: Option<BranchPatch>,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(branch) = branch {
            self.patch_branch_placeholder_to_current(branch, "fallible recover done target")?;
        }
        Ok(())
    }
}
