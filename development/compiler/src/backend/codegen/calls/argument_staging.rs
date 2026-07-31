use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_staged_scalar_arguments(
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
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::U8(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_u8_value_to_w(value, WReg::W16)?;
                    self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::Usize(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_usize_value_to_x(value, XReg::X16)?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::Bool(value) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_bool_value_to_w(value, WReg::W16)?;
                    self.encoder.emit_str_w_sp(WReg::W16, slot.offset());
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::Str(value) => {
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_word_index = next_abi_word_index(abi_word_index, "call str argument")?;
                    let len_slot = staging_slot(frame, len_word_index)?;
                    self.emit_str_value_to_x_pair(value, XReg::X16, XReg::X17)?;
                    self.encoder.emit_str_x_sp(XReg::X16, ptr_slot.offset());
                    self.encoder.emit_str_x_sp(XReg::X17, len_slot.offset());
                    abi_word_index =
                        advance_abi_word_index(abi_word_index, 2, "call argument index")?;
                }
                ScalarArgument::Slice(value) => {
                    let ptr_slot = staging_slot(frame, abi_word_index)?;
                    let len_word_index =
                        next_abi_word_index(abi_word_index, "call slice argument")?;
                    let len_slot = staging_slot(frame, len_word_index)?;
                    self.emit_slice_value_to_x_pair(value, XReg::X16, XReg::X17)?;
                    self.encoder.emit_str_x_sp(XReg::X16, ptr_slot.offset());
                    self.encoder.emit_str_x_sp(XReg::X17, len_slot.offset());
                    abi_word_index =
                        advance_abi_word_index(abi_word_index, 2, "call argument index")?;
                }
                ScalarArgument::Borrow(argument) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_borrow_source_address_to_x(argument.source, XReg::X16, frame)?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::AggregateIndirect(argument) => {
                    let slot = staging_slot(frame, abi_word_index)?;
                    self.emit_aggregate_argument_source_address_to_x(
                        argument.source,
                        XReg::X16,
                        frame,
                    )?;
                    self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::AggregateDirect(argument) => {
                    if argument.words != argument.layout.size.div_ceil(8) as usize {
                        return Err(vec![Diagnostic::error(
                            "E9003",
                            "direct aggregate argument must use full ABI words",
                        )]);
                    }
                    for word_index in 0..argument.words {
                        let register_index = advance_abi_word_index(
                            abi_word_index,
                            word_index,
                            "direct aggregate argument index",
                        )?;
                        let slot = staging_slot(frame, register_index)?;
                        self.emit_direct_aggregate_argument_word_to_staging_slot(
                            argument.source,
                            argument.layout,
                            word_index,
                            slot,
                            frame,
                        )?;
                    }
                    abi_word_index = advance_abi_word_index(
                        abi_word_index,
                        argument.words,
                        "call argument index",
                    )?;
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
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
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
                    abi_word_index = next_abi_word_index(abi_word_index, "call argument index")?;
                }
                ScalarArgument::AggregateDirect(argument) => {
                    for word_index in 0..argument.words {
                        let register_index = advance_abi_word_index(
                            abi_word_index,
                            word_index,
                            "direct aggregate argument index",
                        )?;
                        let slot = staging_slot(frame, register_index)?;
                        self.emit_staged_x_argument_word(
                            register_index,
                            slot,
                            outgoing_stack.area_size,
                        )?;
                    }
                    abi_word_index = advance_abi_word_index(
                        abi_word_index,
                        argument.words,
                        "call argument index",
                    )?;
                }
                ScalarArgument::Str(_) | ScalarArgument::Slice(_) => {
                    let len_word_index = next_abi_word_index(abi_word_index, "view argument")?;
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
                    abi_word_index =
                        advance_abi_word_index(abi_word_index, 2, "call argument index")?;
                }
            }
        }

        Ok(outgoing_stack)
    }

    pub(in crate::backend::codegen) fn emit_staged_w_argument_word(
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

    pub(in crate::backend::codegen) fn emit_staged_x_argument_word(
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

    pub(in crate::backend::codegen) fn emit_restore_outgoing_stack_arguments(
        &mut self,
        outgoing_stack: OutgoingStackArguments,
    ) {
        if outgoing_stack.area_size > 0 {
            self.encoder.emit_add_sp_imm(outgoing_stack.area_size);
        }
    }

    pub(in crate::backend::codegen) fn emit_scalar_spills(
        &mut self,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            if let Some(register) = XReg::local(slot.local_index()) {
                self.encoder.emit_str_x_sp(register, slot.offset());
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_scalar_reloads(
        &mut self,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for slot in frame.scalar_spill_slots() {
            if let Some(register) = XReg::local(slot.local_index()) {
                self.encoder.emit_ldr_x_sp(register, slot.offset());
            }
        }

        Ok(())
    }
}
