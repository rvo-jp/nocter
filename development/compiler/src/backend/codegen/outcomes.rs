use super::*;
use crate::outcomes::OutcomeLayer;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_call_composed_outcome(
        &mut self,
        destination: ComposedOutcomeDestination,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        outer: OutcomeLayer,
        inner: OutcomeLayer,
        outer_mode: &OutcomeFailureMode,
        inner_mode: &OutcomeFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "composed outcome call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;
        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);

        let outer_success = self.branch_when_outcome_tag_is_zero(0)?;
        self.emit_outcome_layer_action(outer, 0, outer_mode, frame, return_type)?;
        let outer_recover_done = self.emit_recover_done_branch_if_needed(outer_mode);
        self.patch_branch_placeholder_to_current(outer_success, "outer outcome success target")?;

        let inner_success = self.branch_when_outcome_tag_is_zero(1)?;
        self.emit_outcome_layer_action(inner, 1, inner_mode, frame, return_type)?;
        let inner_recover_done = self.emit_recover_done_branch_if_needed(inner_mode);
        self.patch_branch_placeholder_to_current(inner_success, "inner outcome success target")?;

        self.emit_composed_payload_to_destination(destination, frame)?;
        self.patch_recover_done_branch(inner_recover_done)?;
        self.patch_recover_done_branch(outer_recover_done)?;
        Ok(())
    }

    fn branch_when_outcome_tag_is_zero(
        &mut self,
        tag_index: usize,
    ) -> Result<control_flow::BranchPatch, Vec<Diagnostic>> {
        let register = XReg::argument(tag_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9002",
                "invalid composed outcome tag register",
            )]
        })?;
        self.encoder.emit_cmp_x_zero(register);
        Ok(self.emit_cond_branch_placeholder(BranchCondition::Eq))
    }

    fn emit_outcome_layer_action(
        &mut self,
        layer: OutcomeLayer,
        tag_index: usize,
        mode: &OutcomeFailureMode,
        frame: &FrameLayout,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        if layer == OutcomeLayer::Fallible && tag_index == 1 {
            self.shift_inner_error_payload_to_standard_registers();
        }
        match (layer, mode) {
            (OutcomeLayer::Fallible, _) => {
                self.emit_fallible_failure_action(mode, frame, return_type)
            }
            (OutcomeLayer::Optional, OutcomeFailureMode::Propagate) => {
                self.emit_return_optional_none(Some(frame), return_type)
            }
            (OutcomeLayer::Optional, OutcomeFailureMode::Trap) => {
                self.emit_trap();
                Ok(())
            }
            (
                OutcomeLayer::Optional,
                OutcomeFailureMode::Handle { instructions }
                | OutcomeFailureMode::Recover { instructions },
            ) => {
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            (
                OutcomeLayer::Optional,
                OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
            ) => {
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                self.emit_return_optional_none(Some(frame), return_type)
            }
            (OutcomeLayer::Optional, OutcomeFailureMode::Catch { .. }) => {
                Err(vec![Diagnostic::error(
                    "E9002",
                    "optional absence cannot enter a catch handler",
                )])
            }
        }
    }

    fn shift_inner_error_payload_to_standard_registers(&mut self) {
        self.encoder.emit_mov_x(XReg::X1, XReg::X2);
        self.encoder.emit_mov_x(XReg::X2, XReg::X3);
        self.encoder.emit_mov_x(XReg::X3, XReg::X4);
        self.encoder.emit_mov_x(XReg::X4, XReg::X5);
        emit_mov_i32_to_w0(&mut self.encoder, 1);
    }

    fn emit_composed_payload_to_destination(
        &mut self,
        destination: ComposedOutcomeDestination,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            ComposedOutcomeDestination::I32(destination) => {
                self.encoder.emit_mov_w(WReg::W16, WReg::W2);
                self.emit_scalar_reloads(frame)?;
                self.emit_w_to_i32_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::U8(destination) => {
                self.encoder.emit_mov_w(WReg::W16, WReg::W2);
                self.emit_scalar_reloads(frame)?;
                self.emit_w_to_u8_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::Usize(destination)
            | ComposedOutcomeDestination::Borrow(destination) => {
                self.encoder.emit_mov_x(XReg::X16, XReg::X2);
                self.emit_scalar_reloads(frame)?;
                self.emit_x_to_usize_location(XReg::X16, destination)
            }
            ComposedOutcomeDestination::Bool(destination) => {
                self.encoder.emit_mov_w(WReg::W16, WReg::W2);
                self.emit_scalar_reloads(frame)?;
                self.emit_w_to_bool_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::Str(destination) => {
                self.encoder.emit_mov_x(XReg::X16, XReg::X2);
                self.encoder.emit_mov_x(XReg::X17, XReg::X3);
                self.emit_scalar_reloads(frame)?;
                self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
            }
            ComposedOutcomeDestination::Slice(destination) => {
                self.encoder.emit_mov_x(XReg::X16, XReg::X2);
                self.encoder.emit_mov_x(XReg::X17, XReg::X3);
                self.emit_scalar_reloads(frame)?;
                self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
            }
        }
    }
}
