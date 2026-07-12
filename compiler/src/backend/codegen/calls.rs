use super::{EntryEmitter, FunctionCallPatch, FunctionSymbol};
use crate::backend::frame::{ArgumentStagingSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, BorrowSource, FallibleFailureMode, I32Location, ScalarArgument, SliceLocation,
    StrLocation, Type, U8Location, UsizeLocation,
};
use crate::target::arm64::{BranchCondition, WReg, XReg};

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
        destination_slot: usize,
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
        self.emit_aggregate_slot_address_to_x(destination_slot, XReg::X8, frame)?;

        self.emit_call(function);
        self.emit_scalar_reloads(frame)?;
        Ok(())
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
                    let source_offset = borrow_source_spill_offset(argument.source, frame)?;
                    self.encoder.emit_add_x_sp_imm(XReg::X16, source_offset);
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index += 1;
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
                ScalarArgument::Usize(_) | ScalarArgument::Borrow(_) => {
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

fn borrow_source_spill_offset(
    source: BorrowSource,
    frame: &FrameLayout,
) -> Result<u32, Vec<Diagnostic>> {
    let Some(local_index) = borrow_source_local_index(source) else {
        return Err(vec![Diagnostic::error(
            "E9005",
            "borrow argument emission requires a scalar local source",
        )]);
    };

    frame
        .scalar_spill_slot(local_index)
        .map(|slot| slot.offset())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                format!("borrow argument source local {local_index} has no spill slot"),
            )]
        })
}

fn borrow_source_local_index(source: BorrowSource) -> Option<usize> {
    match source {
        BorrowSource::I32(I32Location::Local(index)) => Some(index),
        BorrowSource::U8(U8Location::Local(index)) => Some(index),
        BorrowSource::Usize(UsizeLocation::Local(index)) => Some(index),
        BorrowSource::Bool(BoolLocation::Local(index)) => Some(index),
        BorrowSource::I32(I32Location::Return | I32Location::Parameter(_))
        | BorrowSource::U8(U8Location::Return | U8Location::Parameter(_))
        | BorrowSource::Usize(UsizeLocation::Return | UsizeLocation::Parameter(_))
        | BorrowSource::Bool(BoolLocation::Return | BoolLocation::Parameter(_)) => None,
    }
}
