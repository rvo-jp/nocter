use super::{EntryEmitter, FunctionCallPatch, FunctionSymbol};
use crate::abi::ValueLayout;
use crate::backend::frame::{ArgumentStagingSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgumentSource, AggregateLocation, BoolLocation, BorrowSource, FallibleFailureMode,
    I32Location, ScalarArgument, SliceLocation, StrLocation, Type, U8Location, UsizeLocation,
};
use crate::target::arm64::{BranchCondition, MoveWideShift, WReg, XReg};

pub(super) struct FallibleDirectAggregateCall<'a> {
    pub(super) destination: AggregateLocation,
    pub(super) function: FunctionSymbol,
    pub(super) arguments: &'a [ScalarArgument],
    pub(super) layout: ValueLayout,
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

    pub(super) fn emit_tail_call(
        &mut self,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !arguments.is_empty() {
            let Some(frame) = frame else {
                return Err(vec![Diagnostic::error(
                    "E9005",
                    "tail call argument staging requires a stack frame",
                )]);
            };
            self.emit_staged_scalar_arguments(arguments, frame)?;
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;
        self.emit_aggregate_destination_to_x8(destination, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;
        self.emit_aggregate_destination_to_x8(destination, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(call.arguments, frame)?;

        self.emit_call(call.function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
        self.emit_staged_scalar_arguments(arguments, frame)?;

        self.emit_call(function);
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
    ) -> Result<(), Vec<Diagnostic>> {
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

        let mut abi_word_index = 0;
        for argument in arguments {
            match argument {
                ScalarArgument::I32(_) | ScalarArgument::U8(_) | ScalarArgument::Bool(_) => {
                    let Some(register) = WReg::argument(abi_word_index) else {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            format!(
                                "codegen supports at most 8 ABI argument words, got argument word {abi_word_index}"
                            ),
                        )]);
                    };
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.encoder.emit_ldr_w_sp(register, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::Usize(_)
                | ScalarArgument::Borrow(_)
                | ScalarArgument::AggregateIndirect(_) => {
                    let Some(register) = XReg::argument(abi_word_index) else {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            format!(
                                "codegen supports at most 8 ABI argument words, got argument word {abi_word_index}"
                            ),
                        )]);
                    };
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.encoder.emit_ldr_x_sp(register, slot.offset());
                    abi_word_index += 1;
                }
                ScalarArgument::AggregateDirect(argument) => {
                    for word_index in 0..argument.words {
                        let register_index = abi_word_index + word_index;
                        let Some(register) = XReg::argument(register_index) else {
                            return Err(vec![Diagnostic::error(
                                "E9003",
                                format!(
                                    "codegen supports at most 8 ABI argument words, got argument word {register_index}"
                                ),
                            )]);
                        };
                        let slot = staging_slot(frame, register_index)?;
                        self.encoder.emit_ldr_x_sp(register, slot.offset());
                    }
                    abi_word_index += argument.words;
                }
                ScalarArgument::Str(_) | ScalarArgument::Slice(_) => {
                    let Some(ptr_register) = XReg::argument(abi_word_index) else {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            format!(
                                "codegen supports at most 8 ABI argument words, got argument word {abi_word_index}"
                            ),
                        )]);
                    };
                    let len_word_index = abi_word_index + 1;
                    let Some(len_register) = XReg::argument(len_word_index) else {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            format!(
                                "codegen supports at most 8 ABI argument words, got argument word {len_word_index}"
                            ),
                        )]);
                    };
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_slot = staging_slot(frame, len_word_index)?;
                    self.encoder.emit_ldr_x_sp(ptr_register, ptr_slot.offset());
                    self.encoder.emit_ldr_x_sp(len_register, len_slot.offset());
                    abi_word_index += 2;
                }
            }
        }

        Ok(())
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
                format!(
                    "codegen supports at most 8 ABI argument words, got argument word {abi_word_index}"
                ),
            )]
        })?;
    debug_assert_eq!(slot.abi_word_index(), abi_word_index);
    Ok(slot)
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
            BorrowSource::AggregateSlot(slot_index) => frame
                .aggregate_slot(slot_index)
                .map(|slot| slot.offset())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument aggregate slot {slot_index} is not reserved"),
                    )]
                })?,
            BorrowSource::I32(I32Location::Return | I32Location::Parameter(_))
            | BorrowSource::U8(U8Location::Return | U8Location::Parameter(_))
            | BorrowSource::Usize(UsizeLocation::Return | UsizeLocation::Parameter(_))
            | BorrowSource::Bool(BoolLocation::Return | BoolLocation::Parameter(_)) => {
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
        if chunk_bytes < DIRECT_AGGREGATE_WORD_BYTES {
            self.encoder.emit_movz_x(XReg::X16, 0, MoveWideShift::Lsl0);
            self.encoder.emit_str_x_sp(XReg::X16, staging_slot.offset());
        }
        match chunk_bytes {
            DIRECT_AGGREGATE_WORD_BYTES => {
                self.encoder.emit_ldr_x_sp(XReg::X16, source_offset);
                self.encoder.emit_str_x_sp(XReg::X16, staging_slot.offset());
            }
            DIRECT_AGGREGATE_I32_BYTES => {
                self.encoder.emit_ldr_w_sp(WReg::W16, source_offset);
                self.encoder.emit_str_w_sp(WReg::W16, staging_slot.offset());
            }
            DIRECT_AGGREGATE_U8_BYTES => {
                self.encoder.emit_ldrb_w_sp(WReg::W16, source_offset);
                self.encoder
                    .emit_strb_w_sp(WReg::W16, staging_slot.offset());
            }
            _ => {
                return Err(unsupported_direct_aggregate_chunk_diagnostic(
                    chunk_bytes,
                    "direct aggregate argument",
                ));
            }
        }
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
                DIRECT_AGGREGATE_WORD_BYTES => {
                    let register = XReg::argument(register_index).ok_or_else(|| {
                        direct_aggregate_result_diagnostic("result register is unavailable")
                    })?;
                    self.encoder.emit_str_x_sp(register, destination);
                }
                DIRECT_AGGREGATE_I32_BYTES => {
                    let register = WReg::argument(register_index).ok_or_else(|| {
                        direct_aggregate_result_diagnostic("result register is unavailable")
                    })?;
                    self.encoder.emit_str_w_sp(register, destination);
                }
                DIRECT_AGGREGATE_U8_BYTES => {
                    let register = WReg::argument(register_index).ok_or_else(|| {
                        direct_aggregate_result_diagnostic("result register is unavailable")
                    })?;
                    self.encoder.emit_strb_w_sp(register, destination);
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
    if remaining_bytes >= DIRECT_AGGREGATE_WORD_BYTES {
        return Ok(DIRECT_AGGREGATE_WORD_BYTES);
    }
    match remaining_bytes {
        DIRECT_AGGREGATE_I32_BYTES | DIRECT_AGGREGATE_U8_BYTES => Ok(remaining_bytes),
        _ => Err(unsupported_direct_aggregate_chunk_diagnostic(
            remaining_bytes,
            subject,
        )),
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
const DIRECT_AGGREGATE_I32_BYTES: u32 = 4;
const DIRECT_AGGREGATE_U8_BYTES: u32 = 1;
