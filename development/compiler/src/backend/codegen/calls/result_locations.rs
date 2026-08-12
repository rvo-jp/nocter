use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_aggregate_slot_address_to_x(
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

    pub(in crate::backend::codegen) fn emit_aggregate_destination_to_x8(
        &mut self,
        destination: AggregateLocation,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(Some(frame));
                Ok(())
            }
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
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_i32_location(
        &mut self,
        destination: I32Location,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_i32_location(WReg::W0, destination)
    }

    pub(in crate::backend::codegen) fn emit_w_to_i32_location(
        &mut self,
        source: WReg,
        destination: I32Location,
    ) -> Result<(), Vec<Diagnostic>> {
        if let I32Location::Local(index) = destination {
            return self.emit_w_to_local_word(source, index, LocalScalarWidth::I32);
        }

        let destination = self.i32_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_usize_location(
        &mut self,
        destination: UsizeLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_to_usize_location(XReg::X0, destination)
    }

    pub(in crate::backend::codegen) fn emit_x_to_usize_location(
        &mut self,
        source: XReg,
        destination: UsizeLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        if let UsizeLocation::Local(index) = destination {
            return self.emit_x_to_local_word(source, index);
        }

        let destination = self.usize_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_x(destination, source);
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_u8_location(
        &mut self,
        destination: U8Location,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_u8_location(WReg::W0, destination)
    }

    pub(in crate::backend::codegen) fn emit_w_to_u8_location(
        &mut self,
        source: WReg,
        destination: U8Location,
    ) -> Result<(), Vec<Diagnostic>> {
        if let U8Location::Local(index) = destination {
            return self.emit_w_to_local_word(source, index, LocalScalarWidth::Byte);
        }

        let destination = self.u8_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_bool_location(
        &mut self,
        destination: BoolLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_w_to_bool_location(WReg::W0, destination)
    }

    pub(in crate::backend::codegen) fn emit_w_to_bool_location(
        &mut self,
        source: WReg,
        destination: BoolLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        if let BoolLocation::Local(index) = destination {
            return self.emit_w_to_local_word(source, index, LocalScalarWidth::Byte);
        }

        let destination = self.bool_location_register(destination)?;
        if destination != source {
            self.encoder.emit_mov_w(destination, source);
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_str_location(
        &mut self,
        destination: StrLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_pair_to_str_location(XReg::X0, XReg::X1, destination)
    }

    pub(in crate::backend::codegen) fn emit_x_pair_to_str_location(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        destination: StrLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        if let StrLocation::Local(index) = destination {
            return self.emit_x_pair_to_local_words(ptr_source, len_source, index);
        }

        let (ptr_destination, len_destination) = self.str_location_registers(destination)?;
        self.emit_x_pair_to_x_pair(ptr_source, len_source, ptr_destination, len_destination)
    }

    pub(in crate::backend::codegen) fn emit_call_result_to_slice_location(
        &mut self,
        destination: SliceLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_x_pair_to_slice_location(XReg::X0, XReg::X1, destination)
    }

    pub(in crate::backend::codegen) fn emit_x_pair_to_slice_location(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        destination: SliceLocation,
    ) -> Result<(), Vec<Diagnostic>> {
        if let SliceLocation::Local(index) = destination {
            return self.emit_x_pair_to_local_words(ptr_source, len_source, index);
        }

        let (ptr_destination, len_destination) = self.slice_location_registers(destination)?;
        self.emit_x_pair_to_x_pair(ptr_source, len_source, ptr_destination, len_destination)
    }
}
