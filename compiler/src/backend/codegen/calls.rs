use super::{
    DARWIN_SYSCALL_TRAP, EntryEmitter, FunctionCallPatch, FunctionSymbol, emit_mov_i32_to_w,
    emit_mov_u64_to_x,
};
use crate::abi::{ABI_WORD_SIZE, ARGUMENT_REGISTER_COUNT, ValueLayout};
use crate::backend::frame::{ArgumentStagingSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgumentSource, AggregateLocation, BoolLocation, BorrowSource, FallibleFailureMode,
    I32Location, ScalarArgument, SliceLocation, StrLocation, Type, U8Location, UsizeLocation,
    UsizeValue,
};
use crate::target::arm64::{BranchCondition, WReg, XReg};

pub(super) struct FallibleDirectAggregateCall<'a> {
    pub(super) destination: AggregateLocation,
    pub(super) function: FunctionSymbol,
    pub(super) arguments: &'a [ScalarArgument],
    pub(super) layout: ValueLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutgoingStackArguments {
    area_size: u32,
}

impl EntryEmitter {
    pub(super) fn emit_call(&mut self, function: FunctionSymbol) {
        let instruction_offset = self.encoder.position();
        self.encoder.emit_bl(0);
        self.call_patches.push(FunctionCallPatch {
            instruction_offset,
            function,
        });
    }

    pub(super) fn emit_darwin_syscall(
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

    fn emit_staged_syscall_words(
        &mut self,
        number: &UsizeValue,
        arguments: &[UsizeValue],
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_syscall_word_to_staging(0, number, frame)?;
        for (index, argument) in arguments.iter().enumerate() {
            self.emit_syscall_word_to_staging(index + 1, argument, frame)?;
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
            let slot = staging_slot(frame, index + 1)?;
            self.encoder.emit_ldr_x_sp(register, slot.offset());
        }
        Ok(())
    }

    fn emit_syscall_word_to_staging(
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

    fn emit_darwin_syscall_result_to_location(
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

    fn emit_syscall_result_words_to_location(
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

    pub(super) fn emit_tail_call(
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

    pub(super) fn emit_call_void(
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

    pub(super) fn emit_call_fallible_void(
        &mut self,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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

    pub(super) fn emit_call_aggregate(
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

    pub(super) fn emit_call_direct_aggregate(
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

    pub(super) fn emit_call_fallible_aggregate(
        &mut self,
        destination: AggregateLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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

    pub(super) fn emit_call_fallible_direct_aggregate(
        &mut self,
        call: FallibleDirectAggregateCall<'_>,
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.emit_fallible_direct_aggregate_result_to_location(
            call.destination,
            call.layout,
            frame,
        )?;
        self.emit_scalar_reloads(frame)?;
        Ok(())
    }

    fn emit_direct_aggregate_result_to_location(
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

    fn emit_fallible_direct_aggregate_result_to_location(
        &mut self,
        destination: AggregateLocation,
        layout: ValueLayout,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_direct_aggregate_register_layout(layout, "fallible direct aggregate call result")?;

        match destination {
            AggregateLocation::DirectReturn => {
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

    pub(super) fn emit_call_i32(
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

    pub(super) fn emit_call_fallible_i32(
        &mut self,
        destination: I32Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_i32_location(WReg::W16, destination)
    }

    pub(super) fn emit_call_usize(
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

    pub(super) fn emit_call_fallible_usize(
        &mut self,
        destination: UsizeLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_to_usize_location(XReg::X16, destination)
    }

    pub(super) fn emit_call_u8(
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

    pub(super) fn emit_call_fallible_u8(
        &mut self,
        destination: U8Location,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_u8_location(WReg::W16, destination)
    }

    pub(super) fn emit_call_bool(
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

    pub(super) fn emit_call_fallible_bool(
        &mut self,
        destination: BoolLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_w(WReg::W16, WReg::W1);
        self.emit_scalar_reloads(frame)?;
        self.emit_w_to_bool_location(WReg::W16, destination)
    }

    pub(super) fn emit_call_str(
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

    pub(super) fn emit_call_fallible_str(
        &mut self,
        destination: StrLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.encoder.emit_mov_x(XReg::X17, XReg::X2);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(super) fn emit_call_slice(
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

    pub(super) fn emit_call_fallible_slice(
        &mut self,
        destination: SliceLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
        failure_mode: &FallibleFailureMode,
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
        self.patch_branch_placeholder_to_current(success_branch, "fallible call success target")?;
        self.encoder.emit_mov_x(XReg::X16, XReg::X1);
        self.encoder.emit_mov_x(XReg::X17, XReg::X2);
        self.emit_scalar_reloads(frame)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
    }

    fn emit_staged_scalar_arguments(
        &mut self,
        arguments: &[ScalarArgument],
        frame: &FrameLayout,
    ) -> Result<OutgoingStackArguments, Vec<Diagnostic>> {
        let mut abi_word_index = 0;
        for argument in arguments {
            match argument {
                ScalarArgument::I32(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_i32_value_to_w(value, WReg::W16)?;
                    self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::U8(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_u8_value_to_w(value, WReg::W16)?;
                    self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::Usize(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_usize_value_to_x(value, XReg::X16)?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::Bool(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_bool_value_to_w(value, WReg::W16)?;
                    self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::Str(value) => {
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_slot = staging_slot(frame, abi_word_index + 1)?;
                    self.emit_str_value_to_x_pair(value, XReg::X16, XReg::X17)?;
                    self.encoder.emit_str_x_sp(XReg::X16, ptr_slot.offset());
                    self.encoder.emit_str_x_sp(XReg::X17, len_slot.offset());
                    abi_word_index += 2;
                }
                ScalarArgument::Slice(value) => {
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_slot = staging_slot(frame, abi_word_index + 1)?;
                    self.emit_slice_value_to_x_pair(value, XReg::X16, XReg::X17)?;
                    self.encoder.emit_str_x_sp(XReg::X16, ptr_slot.offset());
                    self.encoder.emit_str_x_sp(XReg::X17, len_slot.offset());
                    abi_word_index += 2;
                }
                ScalarArgument::Borrow(argument) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_borrow_source_address_to_x(argument.source, XReg::X16, frame)?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::AggregateIndirect(argument) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_aggregate_argument_source_address_to_x(
                        argument.source,
                        XReg::X16,
                        frame,
                    )?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::AggregateDirect(argument) => {
                    if argument.words != argument.layout.size.div_ceil(8) as usize {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            "direct aggregate argument must use full ABI words",
                        )]);
                    }
                    for word_index in 0..argument.words {
                        let slot = staging_slot(frame, abi_word_index + word_index)?;
                        self.emit_direct_aggregate_argument_word_to_staging_slot(
                            argument.source,
                            argument.layout,
                            word_index,
                            slot,
                            frame,
                        )?;
                    }
                    abi_word_index += argument.words;
                }
            }
        }

        let outgoing_stack = OutgoingStackArguments {
            area_size: outgoing_stack_argument_area_size(abi_word_index)?,
        };
        if outgoing_stack.area_size > 0 {
            self.encoder.emit_sub_sp_imm(outgoing_stack.area_size);
        }

        let mut abi_word_index = 0;
        for argument in arguments {
            match argument {
                ScalarArgument::I32(_) | ScalarArgument::U8(_) | ScalarArgument::Bool(_) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_staged_w_argument_word(
                        abi_word_index,
                        slot,
                        outgoing_stack.area_size,
                    )?;
                    abi_word_index += 1;
                }
                ScalarArgument::Usize(_)
                | ScalarArgument::Borrow(_)
                | ScalarArgument::AggregateIndirect(_) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_staged_x_argument_word(
                        abi_word_index,
                        slot,
                        outgoing_stack.area_size,
                    )?;
                    abi_word_index += 1;
                }
                ScalarArgument::AggregateDirect(argument) => {
                    for word_index in 0..argument.words {
                        let register_index = abi_word_index + word_index;
                        let slot = staging_slot(frame, register_index)?;
                        self.emit_staged_x_argument_word(
                            register_index,
                            slot,
                            outgoing_stack.area_size,
                        )?;
                    }
                    abi_word_index += argument.words;
                }
                ScalarArgument::Str(_) | ScalarArgument::Slice(_) => {
                    let len_word_index = abi_word_index + 1;
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_slot = staging_slot(frame, len_word_index)?;
                    self.emit_staged_x_argument_word(
                        abi_word_index,
                        ptr_slot,
                        outgoing_stack.area_size,
                    )?;
                    self.emit_staged_x_argument_word(
                        len_word_index,
                        len_slot,
                        outgoing_stack.area_size,
                    )?;
                    abi_word_index += 2;
                }
            }
        }

        Ok(outgoing_stack)
    }

    fn emit_staged_w_argument_word(
        &mut self,
        abi_word_index: usize,
        slot: ArgumentStagingSlot,
        stack_area_size: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        let source_offset = staged_argument_slot_offset(slot, stack_area_size)?;
        if let Some(register) = WReg::argument(abi_word_index) {
            self.encoder.emit_ldr_w_sp(register, source_offset);
            return Ok(());
        }

        self.encoder.emit_ldr_w_sp(WReg::W16, source_offset);
        let destination_offset = outgoing_stack_argument_word_offset(abi_word_index)?;
        self.encoder.emit_str_w_sp(WReg::W16, destination_offset);
        Ok(())
    }

    fn emit_staged_x_argument_word(
        &mut self,
        abi_word_index: usize,
        slot: ArgumentStagingSlot,
        stack_area_size: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        let source_offset = staged_argument_slot_offset(slot, stack_area_size)?;
        if let Some(register) = XReg::argument(abi_word_index) {
            self.encoder.emit_ldr_x_sp(register, source_offset);
            return Ok(());
        }

        self.encoder.emit_ldr_x_sp(XReg::X16, source_offset);
        let destination_offset = outgoing_stack_argument_word_offset(abi_word_index)?;
        self.encoder.emit_str_x_sp(XReg::X16, destination_offset);
        Ok(())
    }

    fn emit_restore_outgoing_stack_arguments(&mut self, outgoing_stack: OutgoingStackArguments) {
        if outgoing_stack.area_size > 0 {
            self.encoder.emit_add_sp_imm(outgoing_stack.area_size);
        }
    }

    pub(super) fn emit_scalar_spills(
        &mut self,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            let register = XReg::local(slot.local_index()).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!(
                        "codegen supports at most 7 local scalar bindings, got local {}",
                        slot.local_index()
                    ),
                )]
            })?;
            self.encoder.emit_str_x_sp(register, slot.offset());
        }

        Ok(())
    }

    pub(super) fn emit_scalar_reloads(
        &mut self,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            let register = XReg::local(slot.local_index()).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!(
                        "codegen supports at most 7 local scalar bindings, got local {}",
                        slot.local_index()
                    ),
                )]
            })?;
            self.encoder.emit_ldr_x_sp(register, slot.offset());
        }

        Ok(())
    }

    fn emit_aggregate_slot_address_to_x(
        &mut self,
        slot_index: usize,
        register: XReg,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                format!("aggregate call destination slot {slot_index} is not reserved"),
            )]
        })?;
        self.encoder.emit_add_x_sp_imm(register, slot.offset());
        Ok(())
    }

    fn emit_aggregate_destination_to_x8(
        &mut self,
        destination: AggregateLocation,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => Ok(()),
            AggregateLocation::DirectReturn => Err(vec![Diagnostic::error(
                "E9005",
                "indirect aggregate call cannot target direct return registers",
            )]),
            AggregateLocation::Parameter(_) | AggregateLocation::DirectParameter { .. } => {
                Err(vec![Diagnostic::error(
                    "E9005",
                    "indirect aggregate call cannot target parameter storage",
                )])
            }
            AggregateLocation::Slot(slot_index) => {
                self.emit_aggregate_slot_address_to_x(slot_index, XReg::X8, frame)
            }
        }
    }

    fn emit_call_result_to_i32_location(
        &mut self,
        destination: I32Location,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_i32_location(WReg::W0, destination)
    }

    fn emit_w_to_i32_location(
        &mut self,
        source: WReg,
        destination: I32Location,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    fn emit_call_result_to_usize_location(
        &mut self,
        destination: UsizeLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_to_usize_location(XReg::X0, destination)
    }

    fn emit_x_to_usize_location(
        &mut self,
        source: XReg,
        destination: UsizeLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_x(destination, source);
        }

        Ok(())
    }

    fn emit_call_result_to_u8_location(
        &mut self,
        destination: U8Location,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_u8_location(WReg::W0, destination)
    }

    fn emit_w_to_u8_location(
        &mut self,
        source: WReg,
        destination: U8Location,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.u8_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    fn emit_call_result_to_bool_location(
        &mut self,
        destination: BoolLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_bool_location(WReg::W0, destination)
    }

    fn emit_w_to_bool_location(
        &mut self,
        source: WReg,
        destination: BoolLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.bool_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    fn emit_call_result_to_str_location(
        &mut self,
        destination: StrLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_pair_to_str_location(XReg::X0, XReg::X1, destination)
    }

    pub(super) fn emit_x_pair_to_str_location(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        destination: StrLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        let (ptr_destination, len_destination) = self.str_location_registers(destination)?;
        let len_source = if ptr_destination == len_source {
            self.encoder.emit_mov_x(XReg::X17, len_source);
            XReg::X17
        } else {
            len_source
        };

        if ptr_destination != ptr_source {
            self.encoder.emit_mov_x(ptr_destination, ptr_source);
        }
        if len_destination != len_source {
            self.encoder.emit_mov_x(len_destination, len_source);
        }

        Ok(())
    }

    fn emit_call_result_to_slice_location(
        &mut self,
        destination: SliceLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_pair_to_slice_location(XReg::X0, XReg::X1, destination)
    }

    fn emit_x_pair_to_slice_location(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        destination: SliceLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        let (ptr_destination, len_destination) = self.slice_location_registers(destination)?;
        let len_source = if ptr_destination == len_source {
            self.encoder.emit_mov_x(XReg::X17, len_source);
            XReg::X17
        } else {
            len_source
        };

        if ptr_destination != ptr_source {
            self.encoder.emit_mov_x(ptr_destination, ptr_source);
        }
        if len_destination != len_source {
            self.encoder.emit_mov_x(len_destination, len_source);
        }

        Ok(())
    }
}

fn staging_slot(
    frame: &FrameLayout,
    abi_word_index: usize,
) -> Result<ArgumentStagingSlot, Vec<Diagnostic>> {
    let slot = frame
        .argument_staging_slots()
        .get(abi_word_index)
        .copied()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E9003",
                format!("argument staging slot {abi_word_index} is not reserved"),
            )]
        })?;
    debug_assert_eq!(slot.abi_word_index(), abi_word_index);
    Ok(slot)
}

fn call_argument_abi_word_count(arguments: &[ScalarArgument]) -> usize {
    arguments.iter().map(ScalarArgument::abi_word_count).sum()
}

fn tail_call_has_borrow_argument(arguments: &[ScalarArgument]) -> bool {
    arguments
        .iter()
        .any(|argument| matches!(argument, ScalarArgument::Borrow(_)))
}

fn outgoing_stack_argument_area_size(abi_word_count: usize) -> Result<u32, Vec<Diagnostic>> {
    let Some(stack_words) = abi_word_count.checked_sub(ARGUMENT_REGISTER_COUNT) else {
        return Ok(0);
    };
    let bytes = stack_words
        .checked_mul(ABI_WORD_SIZE as usize)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument byte count overflows"))?;
    let aligned = align_usize(bytes, 16)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument alignment overflows"))?;
    u32::try_from(aligned)
        .map_err(|_error| outgoing_stack_argument_diagnostic("stack argument area exceeds u32"))
}

fn staged_argument_slot_offset(
    slot: ArgumentStagingSlot,
    stack_area_size: u32,
) -> Result<u32, Vec<Diagnostic>> {
    slot.offset()
        .checked_add(stack_area_size)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("staged argument offset overflows"))
}

fn outgoing_stack_argument_word_offset(abi_word_index: usize) -> Result<u32, Vec<Diagnostic>> {
    let stack_word_index = abi_word_index
        .checked_sub(ARGUMENT_REGISTER_COUNT)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("argument word is register-passed"))?;
    let offset = stack_word_index
        .checked_mul(ABI_WORD_SIZE as usize)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument offset overflows"))?;
    u32::try_from(offset)
        .map_err(|_error| outgoing_stack_argument_diagnostic("stack argument offset exceeds u32"))
}

fn align_usize(value: usize, align: usize) -> Option<usize> {
    let mask = align.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

fn outgoing_stack_argument_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9003",
        format!("stack argument emission is invalid: {reason}"),
    )]
}

fn syscall_result_store_offset_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        "syscall result store offset overflows",
    )]
}

impl EntryEmitter {
    fn emit_borrow_source_address_to_x(
        &mut self,
        source: BorrowSource,
        register: XReg,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let offset = match source {
            BorrowSource::I32(I32Location::Local(index))
            | BorrowSource::U8(U8Location::Local(index))
            | BorrowSource::Usize(UsizeLocation::Local(index))
            | BorrowSource::Bool(BoolLocation::Local(index)) => frame
                .scalar_spill_slot(index)
                .map(|slot| slot.offset())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument source local {index} has no spill slot"),
                    )]
                })?,
            BorrowSource::I32(I32Location::Parameter(index))
            | BorrowSource::U8(U8Location::Parameter(index))
            | BorrowSource::Usize(UsizeLocation::Parameter(index))
            | BorrowSource::Bool(BoolLocation::Parameter(index)) => frame
                .parameter_spill_slot(index)
                .map(|slot| slot.offset())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument source parameter {index} has no spill slot"),
                    )]
                })?,
            BorrowSource::AggregateSlot(slot_index) => frame
                .aggregate_slot(slot_index)
                .map(|slot| slot.offset())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument aggregate slot {slot_index} is not reserved"),
                    )]
                })?,
            BorrowSource::AggregateParameter(index) => {
                self.emit_parameter_word_to_x(index, register)?;
                return Ok(());
            }
            BorrowSource::I32(I32Location::Return)
            | BorrowSource::U8(U8Location::Return)
            | BorrowSource::Usize(UsizeLocation::Return)
            | BorrowSource::Bool(BoolLocation::Return) => {
                return Err(vec![Diagnostic::error(
                    "E9005",
                    "borrow argument emission requires a local source",
                )]);
            }
        };

        self.encoder.emit_add_x_sp_imm(register, offset);
        Ok(())
    }

    fn emit_aggregate_argument_source_address_to_x(
        &mut self,
        source: AggregateArgumentSource,
        register: XReg,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match source {
            AggregateArgumentSource::Slot(slot_index) => {
                self.emit_aggregate_slot_address_to_x(slot_index, register, frame)
            }
        }
    }

    fn emit_direct_aggregate_argument_word_to_staging_slot(
        &mut self,
        source: AggregateArgumentSource,
        layout: ValueLayout,
        word_index: usize,
        staging_slot: ArgumentStagingSlot,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let AggregateArgumentSource::Slot(slot_index) = source;
        let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                format!("direct aggregate argument source slot {slot_index} is not reserved"),
            )]
        })?;
        let layout_size = u32::try_from(layout.size).map_err(|_error| {
            vec![Diagnostic::error(
                "E9005",
                "direct aggregate argument size exceeds u32 range",
            )]
        })?;
        if slot.size() != layout_size {
            return Err(vec![Diagnostic::error(
                "E9005",
                "direct aggregate argument source slot size does not match layout",
            )]);
        }
        let offset = u32::try_from(word_index)
            .ok()
            .and_then(|word_index| word_index.checked_mul(8))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9005",
                    "direct aggregate argument word offset overflows",
                )]
            })?;
        let remaining_bytes = layout_size.checked_sub(offset).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                "direct aggregate argument word offset exceeds layout size",
            )]
        })?;
        let chunk_bytes =
            direct_aggregate_chunk_bytes(remaining_bytes, "direct aggregate argument")?;
        let source_offset = slot.offset().checked_add(offset).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                "direct aggregate argument source offset overflows",
            )]
        })?;
        self.emit_aggregate_copy_stack_chunk_to_scratch(source_offset, chunk_bytes)?;
        self.encoder.emit_str_x_sp(XReg::X16, staging_slot.offset());
        Ok(())
    }

    fn emit_direct_aggregate_registers_to_stack(
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

fn validate_direct_aggregate_register_layout(
    layout: ValueLayout,
    subject: &str,
) -> Result<(), Vec<Diagnostic>> {
    if layout.size > 16 {
        return Err(direct_aggregate_diagnostic(
            subject,
            "value exceeds two ABI words",
        ));
    }

    let layout_size = u32::try_from(layout.size)
        .map_err(|_error| direct_aggregate_diagnostic(subject, "size exceeds u32 range"))?;
    let mut offset = 0_u32;
    while offset < layout_size {
        let remaining_bytes = layout_size
            .checked_sub(offset)
            .ok_or_else(|| direct_aggregate_diagnostic(subject, "offset exceeds layout size"))?;
        let chunk_bytes = direct_aggregate_chunk_bytes(remaining_bytes, subject)?;
        offset = offset
            .checked_add(chunk_bytes)
            .ok_or_else(|| direct_aggregate_diagnostic(subject, "offset overflows"))?;
    }
    Ok(())
}

fn direct_aggregate_chunk_bytes(
    remaining_bytes: u32,
    subject: &str,
) -> Result<u32, Vec<Diagnostic>> {
    match remaining_bytes {
        0 => Err(unsupported_direct_aggregate_chunk_diagnostic(
            remaining_bytes,
            subject,
        )),
        1..=DIRECT_AGGREGATE_WORD_BYTES => Ok(remaining_bytes),
        _ => Ok(DIRECT_AGGREGATE_WORD_BYTES),
    }
}

fn unsupported_direct_aggregate_chunk_diagnostic(
    chunk_bytes: u32,
    subject: &str,
) -> Vec<Diagnostic> {
    direct_aggregate_diagnostic(
        subject,
        &format!("partial ABI word size {chunk_bytes} is not supported"),
    )
}

fn direct_aggregate_result_diagnostic(reason: &str) -> Vec<Diagnostic> {
    direct_aggregate_diagnostic("direct aggregate result", reason)
}

fn direct_aggregate_diagnostic(subject: &str, reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("{subject} is invalid: {reason}"),
    )]
}

const DIRECT_AGGREGATE_WORD_BYTES: u32 = 8;
