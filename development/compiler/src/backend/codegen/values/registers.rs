use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen::values) fn scalar_local_offset(
        &self,
        local_index: usize,
    ) -> Result<u32, Vec<Diagnostic>> {
        self.current_scalar_spill_offsets
            .get(&local_index)
            .copied()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9005",
                    format!("local ABI word {local_index} has no stack slot"),
                )]
            })
    }

    pub(in crate::backend::codegen::values) fn emit_local_word_to_x(
        &mut self,
        local_index: usize,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(source) = XReg::local(local_index) {
            if source != destination {
                self.encoder.emit_mov_x(destination, source);
            }
            return Ok(());
        }

        let offset = self.scalar_local_offset(local_index)?;
        self.encoder.emit_ldr_x_sp(destination, offset);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_local_word_to_w(
        &mut self,
        local_index: usize,
        destination: WReg,
        width: LocalScalarWidth,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(source) = WReg::local(local_index) {
            if source != destination {
                self.encoder.emit_mov_w(destination, source);
            }
            return Ok(());
        }

        let offset = self.scalar_local_offset(local_index)?;
        match width {
            LocalScalarWidth::I32 => self.encoder.emit_ldr_w_sp(destination, offset),
            LocalScalarWidth::Byte => self.encoder.emit_ldrb_w_sp(destination, offset),
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_x_to_local_word(
        &mut self,
        source: XReg,
        local_index: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(destination) = XReg::local(local_index) {
            if destination != source {
                self.encoder.emit_mov_x(destination, source);
            }
            return Ok(());
        }

        let offset = self.scalar_local_offset(local_index)?;
        self.encoder.emit_str_x_sp(source, offset);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_w_to_local_word(
        &mut self,
        source: WReg,
        local_index: usize,
        width: LocalScalarWidth,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(destination) = WReg::local(local_index) {
            if destination != source {
                self.encoder.emit_mov_w(destination, source);
            }
            return Ok(());
        }

        let offset = self.scalar_local_offset(local_index)?;
        match width {
            LocalScalarWidth::I32 => self.encoder.emit_str_w_sp(source, offset),
            LocalScalarWidth::Byte => self.encoder.emit_strb_w_sp(source, offset),
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_local_word_pair_to_x_pair(
        &mut self,
        first_index: usize,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_index = pair_len_index(first_index, "local view")?;
        if XReg::local(len_index) == Some(ptr_destination) {
            let ptr_source = XReg::local(first_index);
            let scratch =
                pair_scratch_register(&[ptr_destination, ptr_source.unwrap_or(ptr_destination)])?;
            self.emit_local_word_to_x(len_index, scratch)?;
            self.emit_local_word_to_x(first_index, ptr_destination)?;
            if len_destination != scratch {
                self.encoder.emit_mov_x(len_destination, scratch);
            }
            return Ok(());
        }

        self.emit_local_word_to_x(first_index, ptr_destination)?;
        self.emit_local_word_to_x(len_index, len_destination)
    }

    pub(in crate::backend::codegen::values) fn emit_parameter_word_pair_to_x_pair(
        &mut self,
        first_index: usize,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_index = pair_len_index(first_index, "parameter view")?;
        if XReg::argument(len_index) == Some(ptr_destination) {
            let ptr_source = XReg::argument(first_index);
            let scratch =
                pair_scratch_register(&[ptr_destination, ptr_source.unwrap_or(ptr_destination)])?;
            self.emit_parameter_word_to_x(len_index, scratch)?;
            self.emit_parameter_word_to_x(first_index, ptr_destination)?;
            if len_destination != scratch {
                self.encoder.emit_mov_x(len_destination, scratch);
            }
            return Ok(());
        }

        self.emit_parameter_word_to_x(first_index, ptr_destination)?;
        self.emit_parameter_word_to_x(len_index, len_destination)
    }

    pub(in crate::backend::codegen) fn emit_x_pair_to_local_words(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        first_index: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_index = pair_len_index(first_index, "local view destination")?;
        let len_source = if XReg::local(first_index) == Some(len_source) {
            let scratch = pair_scratch_register(&[ptr_source, len_source])?;
            self.encoder.emit_mov_x(scratch, len_source);
            scratch
        } else {
            len_source
        };

        self.emit_x_to_local_word(ptr_source, first_index)?;
        self.emit_x_to_local_word(len_source, len_index)
    }

    pub(in crate::backend::codegen) fn emit_x_pair_to_x_pair(
        &mut self,
        ptr_source: XReg,
        len_source: XReg,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_source = if ptr_destination == len_source {
            let scratch = pair_scratch_register(&[ptr_source, ptr_destination])?;
            self.encoder.emit_mov_x(scratch, len_source);
            scratch
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

    pub(in crate::backend::codegen::values) fn i32_register_destination(
        &self,
        destination: I32Location,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            I32Location::Local(index) => Ok(WReg::local(index)),
            _ => self.i32_location_register(destination).map(Some),
        }
    }

    pub(in crate::backend::codegen::values) fn usize_register_destination(
        &self,
        destination: UsizeLocation,
    ) -> Result<Option<XReg>, Vec<Diagnostic>> {
        match destination {
            UsizeLocation::Local(index) => Ok(XReg::local(index)),
            _ => self.usize_location_register(destination).map(Some),
        }
    }

    pub(in crate::backend::codegen::values) fn u8_register_destination(
        &self,
        destination: U8Location,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            U8Location::Local(index) => Ok(WReg::local(index)),
            _ => self.u8_location_register(destination).map(Some),
        }
    }

    pub(in crate::backend::codegen::values) fn bool_register_destination(
        &self,
        destination: BoolLocation,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            BoolLocation::Local(index) => Ok(WReg::local(index)),
            _ => self.bool_location_register(destination).map(Some),
        }
    }

    pub(in crate::backend::codegen::values) fn i32_register_destination_or_scratch(
        &self,
        destination: I32Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .i32_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    pub(in crate::backend::codegen::values) fn usize_register_destination_or_scratch(
        &self,
        destination: UsizeLocation,
    ) -> Result<XReg, Vec<Diagnostic>> {
        Ok(self
            .usize_register_destination(destination)?
            .unwrap_or(XReg::X16))
    }

    pub(in crate::backend::codegen::values) fn u8_register_destination_or_scratch(
        &self,
        destination: U8Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .u8_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    pub(in crate::backend::codegen::values) fn bool_register_destination_or_scratch(
        &self,
        destination: BoolLocation,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .bool_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    pub(in crate::backend::codegen) fn emit_set_i32(
        &mut self,
        destination: I32Location,
        value: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.i32_register_destination_or_scratch(destination)?;
        self.emit_i32_value_to_w(value, destination_register)?;
        self.emit_w_to_i32_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_usize(
        &mut self,
        destination: UsizeLocation,
        value: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.usize_register_destination_or_scratch(destination)?;
        self.emit_usize_value_to_x(value, destination_register)?;
        self.emit_x_to_usize_location(destination_register, destination)
    }
}
