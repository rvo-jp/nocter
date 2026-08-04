use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_call_aggregate(
        &mut self,
        destination: AggregateLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "normal aggregate call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_aggregate_destination_to_x8(destination, frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_direct_aggregate(
        &mut self,
        destination: AggregateLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        layout: crate::abi::ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "direct aggregate call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.emit_direct_aggregate_result_to_location(destination, layout, frame)?;
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_fallible_aggregate(
        &mut self,
        destination: AggregateLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible aggregate call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_aggregate_destination_to_x8(destination, frame)?;
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

    pub(in crate::backend::codegen) fn emit_call_fallible_direct_aggregate(
        &mut self,
        call: OutcomeDirectAggregateCall<'_>,
        frame: Option<&FrameLayout>,
        failure_mode: &OutcomeFailureMode,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "fallible direct aggregate call emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        let outgoing_stack = self.emit_staged_scalar_arguments(call.arguments, frame)?;

        self.emit_call(call.function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);
        self.encoder.emit_cmp_x_zero(XReg::X0);
        let success_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done_branch = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.emit_fallible_direct_aggregate_result_to_location(
            call.destination,
            call.layout,
            frame,
        )?;
        self.patch_recover_done_branch(recover_done_branch)?;
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_direct_aggregate_result_to_location(
        &mut self,
        destination: AggregateLocation,
        layout: ValueLayout,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_direct_aggregate_register_layout(layout, "direct aggregate call result")?;
        match destination {
            AggregateLocation::DirectReturn => Ok(()),
            AggregateLocation::Slot(slot_index) => {
                let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("direct aggregate destination slot {slot_index} is not reserved"),
                    )]
                })?;
                let layout_size = u32::try_from(layout.size).map_err(|_error| {
                    vec![Diagnostic::error(
                        "E9005",
                        "direct aggregate size exceeds u32 range",
                    )]
                })?;
                if slot.size() != layout_size {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "direct aggregate destination slot size does not match layout",
                    )]);
                }
                self.emit_direct_aggregate_registers_to_stack(0, layout, slot.offset())?;
                Ok(())
            }
            AggregateLocation::Return => Err(vec![Diagnostic::error(
                "E9005",
                "direct aggregate call cannot target indirect return storage",
            )]),
            AggregateLocation::Parameter(_) | AggregateLocation::DirectParameter { .. } => {
                Err(vec![Diagnostic::error(
                    "E9005",
                    "direct aggregate call cannot target parameter storage",
                )])
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_fallible_direct_aggregate_result_to_location(
        &mut self,
        destination: AggregateLocation,
        layout: ValueLayout,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_direct_aggregate_register_layout(layout, "fallible direct aggregate call result")?;

        match destination {
            AggregateLocation::DirectReturn => {
                if layout.size == 0 {
                    return Ok(());
                }
                self.encoder.emit_mov_x(XReg::X0, XReg::X1);
                if layout.size > 8 {
                    self.encoder.emit_mov_x(XReg::X1, XReg::X2);
                }
                Ok(())
            }
            AggregateLocation::Slot(slot_index) => {
                let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!(
                            "fallible direct aggregate destination slot {slot_index} is not reserved"
                        ),
                    )]
                })?;
                let layout_size = u32::try_from(layout.size).map_err(|_error| {
                    vec![Diagnostic::error(
                        "E9005",
                        "fallible direct aggregate size exceeds u32 range",
                    )]
                })?;
                if slot.size() != layout_size {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "fallible direct aggregate destination slot size does not match layout",
                    )]);
                }
                self.emit_direct_aggregate_registers_to_stack(1, layout, slot.offset())?;
                Ok(())
            }
            AggregateLocation::Return => Err(vec![Diagnostic::error(
                "E9005",
                "fallible direct aggregate call cannot target indirect return storage",
            )]),
            AggregateLocation::Parameter(_) | AggregateLocation::DirectParameter { .. } => {
                Err(vec![Diagnostic::error(
                    "E9005",
                    "fallible direct aggregate call cannot target parameter storage",
                )])
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_direct_aggregate_registers_to_stack(
        &mut self,
        first_register_index: usize,
        layout: ValueLayout,
        destination_offset: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        let layout_size = u32::try_from(layout.size).map_err(|_error| {
            vec![Diagnostic::error(
                "E9005",
                "direct aggregate result size exceeds u32 range",
            )]
        })?;
        let mut offset = 0_u32;
        while offset < layout_size {
            let remaining_bytes = layout_size.checked_sub(offset).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9005",
                    "direct aggregate result offset exceeds layout size",
                )]
            })?;
            let chunk_bytes =
                direct_aggregate_chunk_bytes(remaining_bytes, "direct aggregate result")?;
            let word_index = usize::try_from(offset / DIRECT_AGGREGATE_WORD_BYTES)
                .map_err(|_error| direct_aggregate_result_diagnostic("word index overflows"))?;
            let register_index = first_register_index
                .checked_add(word_index)
                .ok_or_else(|| direct_aggregate_result_diagnostic("register index overflows"))?;
            let destination = destination_offset.checked_add(offset).ok_or_else(|| {
                direct_aggregate_result_diagnostic("destination offset overflows")
            })?;
            match chunk_bytes {
                1..=DIRECT_AGGREGATE_WORD_BYTES => {
                    let register = XReg::argument(register_index).ok_or_else(|| {
                        direct_aggregate_result_diagnostic("result register is unavailable")
                    })?;
                    self.emit_aggregate_copy_x_to_stack_chunk(register, destination, chunk_bytes)?;
                }
                _ => {
                    return Err(unsupported_direct_aggregate_chunk_diagnostic(
                        chunk_bytes,
                        "direct aggregate result",
                    ));
                }
            }
            offset = offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| direct_aggregate_result_diagnostic("offset overflows"))?;
        }
        Ok(())
    }
}
