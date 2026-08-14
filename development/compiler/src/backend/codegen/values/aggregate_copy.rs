use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_copy_aggregate(
        &mut self,
        destination: AggregateLocation,
        source: AggregateLocation,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_copy_aggregate_range_checked(destination, 0, source, 0, layout, frame, true)
    }

    pub(in crate::backend::codegen) fn emit_copy_aggregate_range(
        &mut self,
        destination: AggregateLocation,
        destination_offset: u32,
        source: AggregateLocation,
        source_offset: u32,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_copy_aggregate_range_checked(
            destination,
            destination_offset,
            source,
            source_offset,
            layout,
            frame,
            false,
        )
    }

    pub(in crate::backend::codegen::values) fn emit_copy_aggregate_range_checked(
        &mut self,
        destination: AggregateLocation,
        destination_offset: u32,
        source: AggregateLocation,
        source_offset: u32,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
        require_exact_slots: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "aggregate copy emission requires a stack frame",
            )]);
        };
        let layout_size = u32::try_from(layout.size)
            .map_err(|_error| aggregate_copy_diagnostic("aggregate size exceeds u32 range"))?;
        let source = self.aggregate_copy_source(source, layout_size, frame)?;

        if matches!(destination, AggregateLocation::DirectReturn) && layout.size > 16 {
            return Err(aggregate_copy_diagnostic(
                "direct aggregate return copy exceeds two ABI words",
            ));
        }
        if require_exact_slots {
            validate_aggregate_copy_destination_exact(
                destination,
                destination_offset,
                layout_size,
                frame,
            )?;
            validate_aggregate_copy_source_exact(source, source_offset, layout_size)?;
        } else {
            validate_aggregate_copy_destination_range(
                destination,
                destination_offset,
                layout_size,
                frame,
            )?;
            validate_aggregate_copy_source_range(source, source_offset, layout_size, frame)?;
        }

        let mut offset = 0_u32;
        while offset < layout_size {
            let remaining = layout_size
                .checked_sub(offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            let absolute_source_offset = source_offset
                .checked_add(offset)
                .ok_or_else(|| aggregate_copy_diagnostic("source range offset overflows"))?;
            let absolute_destination_offset = destination_offset
                .checked_add(offset)
                .ok_or_else(|| aggregate_copy_diagnostic("destination range offset overflows"))?;
            self.emit_aggregate_copy_source_chunk_to_scratch(
                source,
                absolute_source_offset,
                chunk_bytes,
            )?;
            self.emit_aggregate_copy_scratch_to_destination(
                destination,
                absolute_destination_offset,
                chunk_bytes,
                frame,
            )?;
            offset = offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn aggregate_copy_source(
        &self,
        source: AggregateLocation,
        _layout_size: u32,
        frame: &FrameLayout,
    ) -> Result<AggregateCopySource, Vec<Diagnostic>> {
        match source {
            AggregateLocation::Slot(source_slot_index) => {
                let source_slot = frame.aggregate_slot(source_slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("aggregate copy source slot {source_slot_index} is not reserved"),
                    )]
                })?;
                Ok(AggregateCopySource::Slot(source_slot))
            }
            AggregateLocation::Parameter(index) => {
                if let Some(register) = XReg::argument(index) {
                    Ok(AggregateCopySource::Parameter(register))
                } else {
                    Ok(AggregateCopySource::StackParameterPointer {
                        parameter_index: index,
                    })
                }
            }
            AggregateLocation::DirectParameter { start_index } => {
                Ok(AggregateCopySource::DirectParameter { start_index })
            }
            AggregateLocation::Borrow(location) => Ok(AggregateCopySource::Borrow(location)),
            AggregateLocation::Return | AggregateLocation::DirectReturn => Err(
                aggregate_copy_diagnostic("aggregate copy cannot read from return locations"),
            ),
        }
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_source_chunk_to_scratch(
        &mut self,
        source: AggregateCopySource,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match source {
            AggregateCopySource::Slot(slot) => {
                let source_offset = slot
                    .offset()
                    .checked_add(offset)
                    .ok_or_else(|| aggregate_copy_diagnostic("source offset overflows"))?;
                self.emit_aggregate_copy_stack_chunk_to_scratch(source_offset, chunk_bytes)?;
            }
            AggregateCopySource::Parameter(register) => {
                self.emit_aggregate_copy_memory_chunk_to_scratch(register, offset, chunk_bytes)?;
            }
            AggregateCopySource::StackParameterPointer { parameter_index } => {
                self.emit_parameter_word_to_x(parameter_index, XReg::X17)?;
                self.emit_aggregate_copy_memory_chunk_to_scratch(XReg::X17, offset, chunk_bytes)?;
            }
            AggregateCopySource::DirectParameter { start_index } => {
                self.emit_direct_aggregate_parameter_chunk_to_scratch(
                    start_index,
                    offset,
                    chunk_bytes,
                )?;
            }
            AggregateCopySource::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X17)?;
                self.emit_aggregate_copy_memory_chunk_to_scratch(XReg::X17, offset, chunk_bytes)?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_scratch_to_destination(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        chunk_bytes: u32,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => {
                self.emit_indirect_return_pointer_to_x8(Some(frame));
                self.emit_aggregate_copy_scratch_to_memory_chunk(XReg::X8, offset, chunk_bytes)
            }
            AggregateLocation::DirectReturn => self.emit_x_to_direct_aggregate_return(offset),
            AggregateLocation::Slot(destination_slot_index) => {
                let destination_slot =
                    frame
                        .aggregate_slot(destination_slot_index)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "E9005",
                                format!(
                                    "aggregate copy destination slot {destination_slot_index} is not reserved"
                                ),
                            )]
                        })?;
                let destination_offset = destination_slot
                    .offset()
                    .checked_add(offset)
                    .ok_or_else(|| aggregate_copy_diagnostic("destination offset overflows"))?;
                self.emit_aggregate_copy_scratch_to_stack_chunk(destination_offset, chunk_bytes)
            }
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.emit_aggregate_copy_scratch_to_memory_chunk(base, offset, chunk_bytes)
            }
            AggregateLocation::Borrow(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), XReg::X8)?;
                self.emit_aggregate_copy_scratch_to_memory_chunk(XReg::X8, offset, chunk_bytes)
            }
            AggregateLocation::DirectParameter { .. } => Err(aggregate_copy_diagnostic(
                "aggregate copy cannot target direct parameter locations",
            )),
        }
    }

    pub(in crate::backend::codegen) fn emit_aggregate_copy_stack_chunk_to_scratch(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if !aggregate_copy_chunk_has_aligned_offset(offset, chunk_bytes) {
            return self.emit_aggregate_copy_stack_bytes_to_scratch(offset, chunk_bytes);
        }

        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_ldr_x_sp(XReg::X16, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_ldr_w_sp(WReg::W16, offset),
            AGGREGATE_U16_STORE_BYTES => self.encoder.emit_ldrh_w_sp(WReg::W16, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_ldrb_w_sp(WReg::W16, offset),
            _ => return self.emit_aggregate_copy_stack_bytes_to_scratch(offset, chunk_bytes),
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_aggregate_copy_memory_chunk_to_scratch(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if !aggregate_copy_chunk_has_aligned_offset(offset, chunk_bytes) {
            return self.emit_aggregate_copy_memory_bytes_to_scratch(base, offset, chunk_bytes);
        }

        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_ldr_x_imm(XReg::X16, base, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_ldr_w_imm(WReg::W16, base, offset),
            AGGREGATE_U16_STORE_BYTES => self.encoder.emit_ldrh_w_imm(WReg::W16, base, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_ldrb_w_imm(WReg::W16, base, offset),
            _ => {
                return self.emit_aggregate_copy_memory_bytes_to_scratch(base, offset, chunk_bytes);
            }
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_aggregate_copy_scratch_to_stack_chunk(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if !aggregate_copy_chunk_has_aligned_offset(offset, chunk_bytes) {
            return self.emit_aggregate_copy_scratch_to_stack_bytes(offset, chunk_bytes);
        }

        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_str_x_sp(XReg::X16, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_str_w_sp(WReg::W16, offset),
            AGGREGATE_U16_STORE_BYTES => self.encoder.emit_strh_w_sp(WReg::W16, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_strb_w_sp(WReg::W16, offset),
            _ => return self.emit_aggregate_copy_scratch_to_stack_bytes(offset, chunk_bytes),
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_aggregate_copy_scratch_to_memory_chunk(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if !aggregate_copy_chunk_has_aligned_offset(offset, chunk_bytes) {
            return self.emit_aggregate_copy_scratch_to_memory_bytes(base, offset, chunk_bytes);
        }

        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_str_x_imm(XReg::X16, base, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_str_w_imm(WReg::W16, base, offset),
            AGGREGATE_U16_STORE_BYTES => self.encoder.emit_strh_w_imm(WReg::W16, base, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_strb_w_imm(WReg::W16, base, offset),
            _ => {
                return self.emit_aggregate_copy_scratch_to_memory_bytes(base, offset, chunk_bytes);
            }
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_aggregate_copy_x_to_stack_chunk(
        &mut self,
        source: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if aggregate_copy_chunk_has_aligned_offset(offset, chunk_bytes) {
            match chunk_bytes {
                AGGREGATE_USIZE_STORE_BYTES => {
                    self.encoder.emit_str_x_sp(source, offset);
                    return Ok(());
                }
                AGGREGATE_I32_STORE_BYTES => {
                    if let Some(source) = w_reg_for_x_reg(source) {
                        self.encoder.emit_str_w_sp(source, offset);
                        return Ok(());
                    }
                }
                AGGREGATE_U16_STORE_BYTES => {
                    if let Some(source) = w_reg_for_x_reg(source) {
                        self.encoder.emit_strh_w_sp(source, offset);
                        return Ok(());
                    }
                }
                AGGREGATE_U8_STORE_BYTES => {
                    if let Some(source) = w_reg_for_x_reg(source) {
                        self.encoder.emit_strb_w_sp(source, offset);
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        if source != XReg::X16 {
            self.encoder.emit_mov_x(XReg::X16, source);
        }
        self.emit_aggregate_copy_scratch_to_stack_chunk(offset, chunk_bytes)
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_stack_bytes_to_scratch(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        self.encoder.emit_movz_x(XReg::X16, 0, MoveWideShift::Lsl0);
        for byte_offset in 0..chunk_bytes {
            let source_offset = offset
                .checked_add(byte_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("source byte offset overflows"))?;
            self.encoder.emit_ldrb_w_sp(WReg::W17, source_offset);
            self.emit_aggregate_copy_byte_to_scratch(byte_offset);
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_memory_bytes_to_scratch(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        self.encoder.emit_movz_x(XReg::X16, 0, MoveWideShift::Lsl0);
        for byte_offset in 0..chunk_bytes {
            let source_offset = offset
                .checked_add(byte_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("source byte offset overflows"))?;
            self.encoder.emit_ldrb_w_imm(WReg::W17, base, source_offset);
            self.emit_aggregate_copy_byte_to_scratch(byte_offset);
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_byte_to_scratch(
        &mut self,
        byte_offset: u32,
    ) {
        if byte_offset != 0 {
            self.encoder
                .emit_lsl_x_imm(XReg::X17, XReg::X17, byte_offset * 8);
        }
        self.encoder.emit_orr_x(XReg::X16, XReg::X16, XReg::X17);
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_scratch_to_stack_bytes(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        for byte_offset in 0..chunk_bytes {
            let destination_offset = offset
                .checked_add(byte_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("destination byte offset overflows"))?;
            self.emit_aggregate_copy_scratch_byte_to_w17(byte_offset);
            self.encoder.emit_strb_w_sp(WReg::W17, destination_offset);
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_scratch_to_memory_bytes(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        for byte_offset in 0..chunk_bytes {
            let destination_offset = offset
                .checked_add(byte_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("destination byte offset overflows"))?;
            self.emit_aggregate_copy_scratch_byte_to_w17(byte_offset);
            self.encoder
                .emit_strb_w_imm(WReg::W17, base, destination_offset);
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_aggregate_copy_scratch_byte_to_w17(
        &mut self,
        byte_offset: u32,
    ) {
        if byte_offset == 0 {
            self.encoder.emit_mov_w(WReg::W17, WReg::W16);
        } else {
            self.encoder
                .emit_lsr_x_imm(XReg::X17, XReg::X16, byte_offset * 8);
        }
    }

    pub(in crate::backend::codegen::values) fn emit_x_to_direct_aggregate_return(
        &mut self,
        offset: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match offset {
            0 => {
                self.encoder.emit_mov_x(XReg::X0, XReg::X16);
                Ok(())
            }
            8 => {
                self.encoder.emit_mov_x(XReg::X1, XReg::X16);
                Ok(())
            }
            _ => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return offset must be 0 or 8",
            )),
        }
    }

    pub(in crate::backend::codegen::values) fn emit_x_to_direct_aggregate_return_chunk(
        &mut self,
        offset: u32,
        byte_count: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        let word_offset = offset - (offset % AGGREGATE_USIZE_STORE_BYTES);
        let byte_offset = offset % AGGREGATE_USIZE_STORE_BYTES;
        if byte_count == 0
            || byte_count > AGGREGATE_USIZE_STORE_BYTES
            || byte_offset + byte_count > AGGREGATE_USIZE_STORE_BYTES
        {
            return Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field crosses an ABI word",
            ));
        }
        if byte_count == AGGREGATE_USIZE_STORE_BYTES {
            return self.emit_x_to_direct_aggregate_return(word_offset);
        }
        let destination = match word_offset {
            0 => XReg::X0,
            8 => XReg::X1,
            _ => {
                return Err(aggregate_store_offset_diagnostic(
                    "direct aggregate return field exceeds two ABI words",
                ));
            }
        };
        self.encoder
            .emit_bfi_x(destination, XReg::X16, byte_offset * 8, byte_count * 8);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn aggregate_parameter_base_register(
        &mut self,
        index: usize,
    ) -> Result<XReg, Vec<Diagnostic>> {
        if self.current_parameter_spill_offsets.contains_key(&index) {
            self.emit_parameter_word_to_x(index, XReg::X17)?;
            return Ok(XReg::X17);
        }
        if let Some(register) = XReg::argument(index) {
            return Ok(register);
        }
        self.emit_parameter_word_to_x(index, XReg::X17)?;
        Ok(XReg::X17)
    }

    pub(in crate::backend::codegen::values) fn emit_direct_aggregate_parameter_chunk_to_w(
        &mut self,
        word_index: usize,
        byte_offset: u32,
        chunk_bytes: u32,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;

        if !self
            .current_parameter_spill_offsets
            .contains_key(&word_index)
            && byte_offset == 0
            && chunk_bytes == AGGREGATE_I32_STORE_BYTES
            && let Some(source) = WReg::argument(word_index)
        {
            if source != destination {
                self.encoder.emit_mov_w(destination, source);
            }
            return Ok(());
        }

        self.emit_parameter_word_to_x(word_index, XReg::X16)?;
        if byte_offset != 0 {
            self.encoder
                .emit_lsr_x_imm(XReg::X16, XReg::X16, byte_offset * 8);
        }
        if chunk_bytes < AGGREGATE_I32_STORE_BYTES {
            let shift = (AGGREGATE_USIZE_STORE_BYTES - chunk_bytes) * 8;
            self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, shift);
            self.encoder.emit_lsr_x_imm(XReg::X16, XReg::X16, shift);
        }
        if destination != WReg::W16 {
            self.encoder.emit_mov_w(destination, WReg::W16);
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_direct_aggregate_parameter_chunk_to_scratch(
        &mut self,
        start_index: usize,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_copy_chunk_bytes(chunk_bytes)?;
        if offset.is_multiple_of(AGGREGATE_USIZE_STORE_BYTES)
            && chunk_bytes == AGGREGATE_USIZE_STORE_BYTES
        {
            let word_index = usize::try_from(offset / AGGREGATE_USIZE_STORE_BYTES)
                .map_err(|_error| aggregate_copy_diagnostic("copy word index overflows"))?;
            let parameter_index = start_index
                .checked_add(word_index)
                .ok_or_else(|| aggregate_copy_diagnostic("copy word index overflows"))?;
            self.emit_parameter_word_to_x(parameter_index, XReg::X16)?;
            return Ok(());
        }

        self.encoder.emit_movz_x(XReg::X16, 0, MoveWideShift::Lsl0);
        for byte_offset in 0..chunk_bytes {
            let source_offset = offset
                .checked_add(byte_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("source byte offset overflows"))?;
            let word_index = usize::try_from(source_offset / AGGREGATE_USIZE_STORE_BYTES)
                .map_err(|_error| aggregate_copy_diagnostic("copy word index overflows"))?;
            let parameter_index = start_index
                .checked_add(word_index)
                .ok_or_else(|| aggregate_copy_diagnostic("copy word index overflows"))?;
            let word_byte_offset = source_offset % AGGREGATE_USIZE_STORE_BYTES;

            self.emit_parameter_word_to_x(parameter_index, XReg::X17)?;
            if word_byte_offset != 0 {
                self.encoder
                    .emit_lsr_x_imm(XReg::X17, XReg::X17, word_byte_offset * 8);
            }
            self.encoder.emit_lsl_x_imm(XReg::X17, XReg::X17, 56);
            self.encoder.emit_lsr_x_imm(XReg::X17, XReg::X17, 56);
            self.emit_aggregate_copy_byte_to_scratch(byte_offset);
        }
        Ok(())
    }
}
