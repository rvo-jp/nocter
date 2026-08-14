use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_store_aggregate_integer(
        &mut self,
        kind: IntegerType,
        destination: AggregateLocation,
        offset: u32,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_count = kind.bit_width() / 8;
        validate_integer_field_offset(offset, byte_count)?;
        self.emit_usize_value_to_x(value, XReg::X16)?;

        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                emit_integer_store_to_base(&mut self.encoder, byte_count, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => {
                self.emit_x_to_direct_aggregate_return_chunk(offset, byte_count)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                emit_integer_store_to_base(&mut self.encoder, byte_count, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                emit_integer_store_to_base(&mut self.encoder, byte_count, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset =
                    self.aggregate_slot_field_offset(slot_index, offset, byte_count, frame)?;
                emit_integer_store_to_stack(&mut self.encoder, byte_count, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_integer(
        &mut self,
        kind: IntegerType,
        destination: UsizeLocation,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_count = kind.bit_width() / 8;
        validate_integer_field_offset(offset, byte_count)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset =
                    self.aggregate_slot_load_offset(source, offset, byte_count, frame)?;
                emit_integer_load_from_stack(&mut self.encoder, byte_count, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                emit_integer_load_from_base(&mut self.encoder, byte_count, base, offset);
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                emit_integer_load_from_base(&mut self.encoder, byte_count, XReg::X8, offset);
            }
            AggregateLocation::DirectParameter { start_index } => {
                if byte_count == AGGREGATE_USIZE_STORE_BYTES {
                    let word_index = direct_aggregate_parameter_word_index(
                        start_index,
                        offset,
                        "integer field",
                    )?;
                    self.emit_parameter_word_to_x(word_index, XReg::X16)?;
                } else {
                    let (word_index, byte_offset) = direct_aggregate_parameter_chunk_source(
                        start_index,
                        offset,
                        byte_count,
                        "integer field",
                    )?;
                    self.emit_direct_aggregate_parameter_chunk_to_w(
                        word_index,
                        byte_offset,
                        byte_count,
                        WReg::W16,
                    )?;
                }
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
        }
        if kind.is_signed() && byte_count < AGGREGATE_USIZE_STORE_BYTES {
            let shift = (AGGREGATE_USIZE_STORE_BYTES - byte_count) * 8;
            self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, shift);
            self.encoder.emit_asr_x_imm(XReg::X16, XReg::X16, shift);
        }
        self.emit_x_to_usize_location(XReg::X16, destination)
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_integer_indexed(
        &mut self,
        kind: IntegerType,
        destination: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_count = kind.bit_width() / 8;
        self.emit_usize_value_to_x(value, XReg::X2)?;
        self.emit_checked_aggregate_index_address_to_x(
            destination,
            base_offset,
            index,
            length,
            stride,
            byte_count,
            XReg::X0,
            frame,
        )?;
        match byte_count {
            1 => self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0),
            2 => self.encoder.emit_strh_w_imm(WReg::W2, XReg::X0, 0),
            4 => self.encoder.emit_str_w_imm(WReg::W2, XReg::X0, 0),
            8 => self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0),
            _ => unreachable!("builtin integer width"),
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_integer_indexed(
        &mut self,
        kind: IntegerType,
        destination: UsizeLocation,
        source: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let byte_count = kind.bit_width() / 8;
        self.emit_checked_aggregate_index_address_to_x(
            source,
            base_offset,
            index,
            length,
            stride,
            byte_count,
            XReg::X8,
            frame,
        )?;
        emit_integer_load_from_base(&mut self.encoder, byte_count, XReg::X8, 0);
        if kind.is_signed() && kind.bit_width() < 64 {
            let shift = 64 - kind.bit_width();
            self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, shift);
            self.encoder.emit_asr_x_imm(XReg::X16, XReg::X16, shift);
        }
        self.emit_x_to_usize_location(XReg::X16, destination)
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_usize(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_usize_field_offset(offset)?;
        self.emit_usize_value_to_x(value, XReg::X16)?;

        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_str_x_imm(XReg::X16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => self.emit_x_to_direct_aggregate_return(offset),
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_str_x_imm(XReg::X16, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder.emit_str_x_imm(XReg::X16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset = self.aggregate_slot_field_offset(
                    slot_index,
                    offset,
                    AGGREGATE_USIZE_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_str_x_sp(XReg::X16, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_i32(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &I32Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(offset)?;
        self.emit_i32_value_to_w(value, WReg::W16)?;

        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => {
                self.emit_x_to_direct_aggregate_return_chunk(offset, AGGREGATE_I32_STORE_BYTES)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_str_w_imm(WReg::W16, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset = self.aggregate_slot_field_offset(
                    slot_index,
                    offset,
                    AGGREGATE_I32_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_str_w_sp(WReg::W16, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_u32(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(offset)?;
        emit_mov_u32_to_w(&mut self.encoder, WReg::W16, value);

        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => {
                self.emit_x_to_direct_aggregate_return_chunk(offset, AGGREGATE_I32_STORE_BYTES)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_str_w_imm(WReg::W16, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset = self.aggregate_slot_field_offset(
                    slot_index,
                    offset,
                    AGGREGATE_I32_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_str_w_sp(WReg::W16, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_u16(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: u16,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_u16_field_offset(offset)?;
        emit_mov_u32_to_w(&mut self.encoder, WReg::W16, u32::from(value));

        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_strh_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => {
                self.emit_x_to_direct_aggregate_return_chunk(offset, AGGREGATE_U16_STORE_BYTES)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_strh_w_imm(WReg::W16, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder.emit_strh_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset = self.aggregate_slot_field_offset(
                    slot_index,
                    offset,
                    AGGREGATE_U16_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_strh_w_sp(WReg::W16, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_u8(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &U8Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_u8_value_to_w(value, WReg::W16)?;
        self.emit_store_aggregate_byte(destination, offset, frame)
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_bool(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &BoolValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_bool_value_to_w(value, WReg::W16)?;
        self.emit_store_aggregate_byte(destination, offset, frame)
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_usize_indexed(
        &mut self,
        destination: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_usize_field_offset(base_offset)?;
        self.emit_usize_value_to_x(value, XReg::X2)?;
        self.emit_checked_aggregate_index_address_to_x(
            destination,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_USIZE_STORE_BYTES,
            XReg::X0,
            frame,
        )?;
        self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_i32_indexed(
        &mut self,
        destination: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        value: &I32Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(base_offset)?;
        self.emit_i32_value_to_w(value, WReg::W2)?;
        self.emit_checked_aggregate_index_address_to_x(
            destination,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_I32_STORE_BYTES,
            XReg::X0,
            frame,
        )?;
        self.encoder.emit_str_w_imm(WReg::W2, XReg::X0, 0);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_u8_indexed(
        &mut self,
        destination: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        value: &U8Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_u8_value_to_w(value, WReg::W2)?;
        self.emit_checked_aggregate_index_address_to_x(
            destination,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_U8_STORE_BYTES,
            XReg::X0,
            frame,
        )?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_store_aggregate_bool_indexed(
        &mut self,
        destination: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        value: &BoolValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_bool_value_to_w(value, WReg::W2)?;
        self.emit_checked_aggregate_index_address_to_x(
            destination,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_U8_STORE_BYTES,
            XReg::X0,
            frame,
        )?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_store_aggregate_byte(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_strb_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => {
                self.emit_x_to_direct_aggregate_return_chunk(offset, AGGREGATE_U8_STORE_BYTES)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_strb_w_imm(WReg::W16, base, offset);
                Ok(())
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder.emit_strb_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_store_offset_diagnostic(
                "direct aggregate parameter stores are not supported",
            )),
            AggregateLocation::Slot(slot_index) => {
                let absolute_offset = self.aggregate_slot_field_offset(
                    slot_index,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_strb_w_sp(WReg::W16, absolute_offset);
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_usize(
        &mut self,
        destination: UsizeLocation,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_usize_field_offset(offset)?;
        let destination_register = self.usize_register_destination_or_scratch(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_USIZE_STORE_BYTES,
                    frame,
                )?;
                self.encoder
                    .emit_ldr_x_sp(destination_register, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder
                    .emit_ldr_x_imm(destination_register, base, offset);
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder
                    .emit_ldr_x_imm(destination_register, XReg::X8, offset);
            }
            AggregateLocation::DirectParameter { start_index } => {
                let word_index =
                    direct_aggregate_parameter_word_index(start_index, offset, "usize field")?;
                self.emit_parameter_word_to_x(word_index, destination_register)?;
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
        }
        self.emit_x_to_usize_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_i32(
        &mut self,
        destination: I32Location,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(offset)?;
        let destination_register = self.i32_register_destination_or_scratch(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_I32_STORE_BYTES,
                    frame,
                )?;
                self.encoder
                    .emit_ldr_w_sp(destination_register, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder
                    .emit_ldr_w_imm(destination_register, base, offset);
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder
                    .emit_ldr_w_imm(destination_register, XReg::X8, offset);
            }
            AggregateLocation::DirectParameter { start_index } => {
                let (word_index, byte_offset) = direct_aggregate_parameter_chunk_source(
                    start_index,
                    offset,
                    AGGREGATE_I32_STORE_BYTES,
                    "i32 field",
                )?;
                self.emit_direct_aggregate_parameter_chunk_to_w(
                    word_index,
                    byte_offset,
                    AGGREGATE_I32_STORE_BYTES,
                    destination_register,
                )?;
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
        }
        self.emit_w_to_i32_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_u8(
        &mut self,
        destination: U8Location,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.u8_register_destination_or_scratch(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    frame,
                )?;
                self.encoder
                    .emit_ldrb_w_sp(destination_register, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder
                    .emit_ldrb_w_imm(destination_register, base, offset);
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder
                    .emit_ldrb_w_imm(destination_register, XReg::X8, offset);
            }
            AggregateLocation::DirectParameter { start_index } => {
                let (word_index, byte_offset) = direct_aggregate_parameter_chunk_source(
                    start_index,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    "u8 field",
                )?;
                self.emit_direct_aggregate_parameter_chunk_to_w(
                    word_index,
                    byte_offset,
                    AGGREGATE_U8_STORE_BYTES,
                    destination_register,
                )?;
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
        }
        self.emit_w_to_u8_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_bool(
        &mut self,
        destination: BoolLocation,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.bool_register_destination_or_scratch(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    frame,
                )?;
                self.encoder
                    .emit_ldrb_w_sp(destination_register, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder
                    .emit_ldrb_w_imm(destination_register, base, offset);
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.encoder
                    .emit_ldrb_w_imm(destination_register, XReg::X8, offset);
            }
            AggregateLocation::DirectParameter { start_index } => {
                let (word_index, byte_offset) = direct_aggregate_parameter_chunk_source(
                    start_index,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    "bool field",
                )?;
                self.emit_direct_aggregate_parameter_chunk_to_w(
                    word_index,
                    byte_offset,
                    AGGREGATE_U8_STORE_BYTES,
                    destination_register,
                )?;
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
        }
        self.emit_w_to_bool_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_usize_indexed(
        &mut self,
        destination: UsizeLocation,
        source: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_usize_field_offset(base_offset)?;
        let destination_register = self.usize_register_destination_or_scratch(destination)?;
        self.emit_checked_aggregate_index_address_to_x(
            source,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_USIZE_STORE_BYTES,
            XReg::X8,
            frame,
        )?;
        self.encoder
            .emit_ldr_x_imm(destination_register, XReg::X8, 0);
        self.emit_x_to_usize_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_i32_indexed(
        &mut self,
        destination: I32Location,
        source: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(base_offset)?;
        let destination_register = self.i32_register_destination_or_scratch(destination)?;
        self.emit_checked_aggregate_index_address_to_x(
            source,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_I32_STORE_BYTES,
            XReg::X8,
            frame,
        )?;
        self.encoder
            .emit_ldr_w_imm(destination_register, XReg::X8, 0);
        self.emit_w_to_i32_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_u8_indexed(
        &mut self,
        destination: U8Location,
        source: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.u8_register_destination_or_scratch(destination)?;
        self.emit_checked_aggregate_index_address_to_x(
            source,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_U8_STORE_BYTES,
            XReg::X8,
            frame,
        )?;
        self.encoder
            .emit_ldrb_w_imm(destination_register, XReg::X8, 0);
        self.emit_w_to_u8_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_aggregate_bool_indexed(
        &mut self,
        destination: BoolLocation,
        source: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.bool_register_destination_or_scratch(destination)?;
        self.emit_checked_aggregate_index_address_to_x(
            source,
            base_offset,
            index,
            length,
            stride,
            AGGREGATE_U8_STORE_BYTES,
            XReg::X8,
            frame,
        )?;
        self.encoder
            .emit_ldrb_w_imm(destination_register, XReg::X8, 0);
        self.emit_w_to_bool_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_checked_aggregate_index_address_to_x(
        &mut self,
        location: AggregateLocation,
        base_offset: u32,
        index: &UsizeValue,
        length: u64,
        stride: u32,
        access_bytes: u32,
        destination: XReg,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if length == 0 || stride == 0 {
            return Err(aggregate_load_diagnostic(
                "indexed aggregate access requires non-empty fixed array metadata",
            ));
        }
        let stack_offset = match location {
            AggregateLocation::Slot(slot_index) => {
                let Some(frame) = frame else {
                    return Err(vec![Diagnostic::error(
                        "E9005",
                        "indexed aggregate access emission requires a stack frame",
                    )]);
                };
                let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("indexed aggregate access slot {slot_index} is not reserved"),
                    )]
                })?;
                Self::validate_aggregate_indexed_access_range(
                    slot,
                    base_offset,
                    length,
                    stride,
                    access_bytes,
                )?;
                Some(slot.offset().checked_add(base_offset).ok_or_else(|| {
                    aggregate_load_diagnostic("indexed aggregate base offset overflows")
                })?)
            }
            AggregateLocation::Parameter(index) => {
                self.emit_parameter_word_to_x(index, destination)?;
                if base_offset != 0 {
                    self.encoder
                        .emit_add_x_imm(destination, destination, base_offset);
                }
                None
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), destination)?;
                if base_offset != 0 {
                    self.encoder
                        .emit_add_x_imm(destination, destination, base_offset);
                }
                None
            }
            _ => {
                return Err(aggregate_load_diagnostic(
                    "indexed aggregate accesses require addressable aggregate storage",
                ));
            }
        };

        let (index_register, scratch_register) = match destination {
            XReg::X16 => (XReg::X17, XReg::X15),
            XReg::X17 => (XReg::X16, XReg::X15),
            _ => (XReg::X16, XReg::X17),
        };
        self.emit_usize_value_to_x(index, index_register)?;
        emit_mov_u64_to_x(&mut self.encoder, scratch_register, length);
        self.emit_index_in_bounds_check(index_register, scratch_register)?;
        if stride > 1 && stride.is_power_of_two() {
            self.encoder
                .emit_lsl_x_imm(index_register, index_register, stride.trailing_zeros());
        } else if stride > 1 {
            emit_mov_u64_to_x(&mut self.encoder, scratch_register, u64::from(stride));
            self.encoder
                .emit_mul_x(index_register, index_register, scratch_register);
        }

        if let Some(stack_offset) = stack_offset {
            self.encoder.emit_add_x_sp_imm(destination, stack_offset);
        }
        self.encoder
            .emit_adds_x(destination, destination, index_register);
        Ok(())
    }

    pub(in crate::backend::codegen) fn aggregate_slot_load_offset(
        &self,
        source: AggregateLocation,
        offset: u32,
        load_bytes: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<u32, Vec<Diagnostic>> {
        let AggregateLocation::Slot(slot_index) = source else {
            return Err(aggregate_load_diagnostic(
                "the native backend can only load aggregate fields from slots",
            ));
        };
        self.aggregate_slot_field_offset(slot_index, offset, load_bytes, frame)
    }

    pub(in crate::backend::codegen::values) fn validate_aggregate_indexed_access_range(
        slot: AggregateSlot,
        base_offset: u32,
        length: u64,
        stride: u32,
        access_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        let last_index = length.checked_sub(1).ok_or_else(|| {
            aggregate_load_diagnostic("indexed aggregate access length underflows")
        })?;
        let last_offset = last_index
            .checked_mul(u64::from(stride))
            .and_then(|offset| offset.checked_add(u64::from(base_offset)))
            .and_then(|offset| offset.checked_add(u64::from(access_bytes)))
            .ok_or_else(|| aggregate_load_diagnostic("indexed aggregate access range overflows"))?;
        if last_offset > u64::from(slot.size()) {
            return Err(aggregate_load_diagnostic(
                "indexed aggregate access range exceeds aggregate slot size",
            ));
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn aggregate_slot_field_offset(
        &self,
        slot_index: usize,
        offset: u32,
        store_bytes: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<u32, Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "aggregate slot store emission requires a stack frame",
            )]);
        };
        let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                format!("aggregate store destination slot {slot_index} is not reserved"),
            )]
        })?;
        let field_end = offset
            .checked_add(store_bytes)
            .ok_or_else(|| aggregate_store_offset_diagnostic("field end overflows"))?;
        if field_end > slot.size() {
            return Err(aggregate_store_offset_diagnostic(
                "field exceeds aggregate slot size",
            ));
        }
        slot.offset()
            .checked_add(offset)
            .ok_or_else(|| aggregate_store_offset_diagnostic("stack offset overflows"))
    }
}

fn validate_integer_field_offset(offset: u32, byte_count: u32) -> Result<(), Vec<Diagnostic>> {
    match byte_count {
        AGGREGATE_U8_STORE_BYTES => Ok(()),
        AGGREGATE_U16_STORE_BYTES => validate_aggregate_u16_field_offset(offset),
        AGGREGATE_I32_STORE_BYTES => validate_aggregate_i32_field_offset(offset),
        AGGREGATE_USIZE_STORE_BYTES => validate_aggregate_usize_field_offset(offset),
        _ => Err(aggregate_store_offset_diagnostic(
            "integer field has an unsupported storage width",
        )),
    }
}

pub(super) fn emit_integer_store_to_base(
    encoder: &mut crate::target::arm64::Encoder,
    byte_count: u32,
    base: XReg,
    offset: u32,
) {
    match byte_count {
        AGGREGATE_U8_STORE_BYTES => encoder.emit_strb_w_imm(WReg::W16, base, offset),
        AGGREGATE_U16_STORE_BYTES => encoder.emit_strh_w_imm(WReg::W16, base, offset),
        AGGREGATE_I32_STORE_BYTES => encoder.emit_str_w_imm(WReg::W16, base, offset),
        AGGREGATE_USIZE_STORE_BYTES => encoder.emit_str_x_imm(XReg::X16, base, offset),
        _ => unreachable!("validated integer storage width"),
    }
}

fn emit_integer_store_to_stack(
    encoder: &mut crate::target::arm64::Encoder,
    byte_count: u32,
    offset: u32,
) {
    match byte_count {
        AGGREGATE_U8_STORE_BYTES => encoder.emit_strb_w_sp(WReg::W16, offset),
        AGGREGATE_U16_STORE_BYTES => encoder.emit_strh_w_sp(WReg::W16, offset),
        AGGREGATE_I32_STORE_BYTES => encoder.emit_str_w_sp(WReg::W16, offset),
        AGGREGATE_USIZE_STORE_BYTES => encoder.emit_str_x_sp(XReg::X16, offset),
        _ => unreachable!("validated integer storage width"),
    }
}

pub(super) fn emit_integer_load_from_base(
    encoder: &mut crate::target::arm64::Encoder,
    byte_count: u32,
    base: XReg,
    offset: u32,
) {
    match byte_count {
        AGGREGATE_U8_STORE_BYTES => encoder.emit_ldrb_w_imm(WReg::W16, base, offset),
        AGGREGATE_U16_STORE_BYTES => encoder.emit_ldrh_w_imm(WReg::W16, base, offset),
        AGGREGATE_I32_STORE_BYTES => encoder.emit_ldr_w_imm(WReg::W16, base, offset),
        AGGREGATE_USIZE_STORE_BYTES => encoder.emit_ldr_x_imm(XReg::X16, base, offset),
        _ => unreachable!("validated integer storage width"),
    }
}

fn emit_integer_load_from_stack(
    encoder: &mut crate::target::arm64::Encoder,
    byte_count: u32,
    offset: u32,
) {
    match byte_count {
        AGGREGATE_U8_STORE_BYTES => encoder.emit_ldrb_w_sp(WReg::W16, offset),
        AGGREGATE_U16_STORE_BYTES => encoder.emit_ldrh_w_sp(WReg::W16, offset),
        AGGREGATE_I32_STORE_BYTES => encoder.emit_ldr_w_sp(WReg::W16, offset),
        AGGREGATE_USIZE_STORE_BYTES => encoder.emit_ldr_x_sp(XReg::X16, offset),
        _ => unreachable!("validated integer storage width"),
    }
}
