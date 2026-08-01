use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_copy_str_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        text: &StrValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "string byte copy emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_str_value_to_x_pair(text, XReg::X1, XReg::X2)?;

        self.encoder.emit_cmp_x_zero(XReg::X2);
        let empty_text = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X4, 1);

        let loop_start = self.encoder.position();
        self.encoder.emit_ldrb_w_imm(WReg::W3, XReg::X1, 0);
        self.encoder.emit_strb_w_imm(WReg::W3, XReg::X0, 0);
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X4);
        self.encoder.emit_adds_x(XReg::X1, XReg::X1, XReg::X4);
        self.encoder.emit_subs_x(XReg::X2, XReg::X2, XReg::X4);
        let has_more = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.patch_branch_placeholder_to_offset(has_more, loop_start, "string copy loop target")?;
        self.patch_branch_placeholder_to_current(empty_text, "string copy empty target")?;

        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_copy_pointer_bytes(
        &mut self,
        destination: &UsizeValue,
        source: &UsizeValue,
        byte_count: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer byte copy emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(destination, XReg::X0)?;
        self.emit_usize_value_to_x(source, XReg::X1)?;
        self.emit_usize_value_to_x(byte_count, XReg::X2)?;

        self.encoder.emit_cmp_x_zero(XReg::X2);
        let empty_copy = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X4, 1);

        let loop_start = self.encoder.position();
        self.encoder.emit_ldrb_w_imm(WReg::W3, XReg::X1, 0);
        self.encoder.emit_strb_w_imm(WReg::W3, XReg::X0, 0);
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X4);
        self.encoder.emit_adds_x(XReg::X1, XReg::X1, XReg::X4);
        self.encoder.emit_subs_x(XReg::X2, XReg::X2, XReg::X4);
        let has_more = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.patch_branch_placeholder_to_offset(has_more, loop_start, "pointer copy loop target")?;
        self.patch_branch_placeholder_to_current(empty_copy, "pointer copy empty target")?;

        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_copy_aggregate_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        source: AggregateLocation,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "aggregate pointer copy emission requires a stack frame",
            )]);
        };
        let layout_size = u32::try_from(layout.size)
            .map_err(|_error| aggregate_copy_diagnostic("aggregate size exceeds u32 range"))?;
        let source = self.aggregate_copy_source(source, layout_size, frame)?;
        validate_aggregate_copy_source_exact(source, 0, layout_size)?;

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X9)?;
        self.emit_usize_value_to_x(offset, XReg::X10)?;
        self.encoder.emit_adds_x(XReg::X9, XReg::X9, XReg::X10);

        let mut chunk_offset = 0_u32;
        while chunk_offset < layout_size {
            let remaining = layout_size
                .checked_sub(chunk_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            self.emit_aggregate_copy_source_chunk_to_scratch(source, chunk_offset, chunk_bytes)?;
            self.emit_aggregate_copy_scratch_to_memory_chunk(XReg::X9, chunk_offset, chunk_bytes)?;
            chunk_offset = chunk_offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }

        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_copy_pointer_to_aggregate(
        &mut self,
        destination: AggregateLocation,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "aggregate pointer take emission requires a stack frame",
            )]);
        };
        let layout_size = u32::try_from(layout.size)
            .map_err(|_error| aggregate_copy_diagnostic("aggregate size exceeds u32 range"))?;
        if layout_size == 0 {
            return Err(aggregate_copy_diagnostic(
                "aggregate pointer take requires a non-empty aggregate layout",
            ));
        }
        validate_aggregate_copy_destination_exact(destination, 0, layout_size, frame)?;

        self.emit_scalar_spills(frame)?;
        self.emit_pointer_offset_address(pointer, offset, XReg::X9)?;
        let mut chunk_offset = 0_u32;
        while chunk_offset < layout_size {
            let remaining = layout_size
                .checked_sub(chunk_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            self.emit_aggregate_copy_memory_chunk_to_scratch(XReg::X9, chunk_offset, chunk_bytes)?;
            self.emit_aggregate_copy_scratch_to_destination(
                destination,
                chunk_offset,
                chunk_bytes,
                frame,
            )?;
            chunk_offset = chunk_offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_load_u8_from_pointer(
        &mut self,
        destination: U8Location,
        pointer: &UsizeValue,
        offset: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_pointer_offset_address(pointer, offset, XReg::X16)?;
        let register = self.u8_register_destination_or_scratch(destination)?;
        self.encoder.emit_ldrb_w_imm(register, XReg::X16, 0);
        self.emit_w_to_u8_location(register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_i32_from_pointer(
        &mut self,
        destination: I32Location,
        pointer: &UsizeValue,
        offset: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_pointer_offset_address(pointer, offset, XReg::X16)?;
        let register = self.i32_register_destination_or_scratch(destination)?;
        self.encoder.emit_ldr_w_imm(register, XReg::X16, 0);
        self.emit_w_to_i32_location(register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_usize_from_pointer(
        &mut self,
        destination: UsizeLocation,
        pointer: &UsizeValue,
        offset: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_pointer_offset_address(pointer, offset, XReg::X16)?;
        let register = self.usize_register_destination_or_scratch(destination)?;
        self.encoder.emit_ldr_x_imm(register, XReg::X16, 0);
        self.emit_x_to_usize_location(register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_bool_from_pointer(
        &mut self,
        destination: BoolLocation,
        pointer: &UsizeValue,
        offset: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_pointer_offset_address(pointer, offset, XReg::X16)?;
        let register = self.bool_register_destination_or_scratch(destination)?;
        self.encoder.emit_ldrb_w_imm(register, XReg::X16, 0);
        self.emit_w_to_bool_location(register, destination)
    }

    pub(in crate::backend::codegen) fn emit_load_str_from_pointer(
        &mut self,
        destination: StrLocation,
        pointer: &UsizeValue,
        offset: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_pointer_offset_address(pointer, offset, XReg::X16)?;
        self.encoder.emit_ldr_x_imm(XReg::X17, XReg::X16, 8);
        self.encoder.emit_ldr_x_imm(XReg::X16, XReg::X16, 0);
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    fn emit_pointer_offset_address(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(pointer, destination)?;
        let offset_register = if destination == XReg::X17 {
            XReg::X16
        } else {
            XReg::X17
        };
        self.emit_usize_value_to_x(offset, offset_register)?;
        self.encoder
            .emit_add_x(destination, destination, offset_register);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_copy_slice_element_to_aggregate(
        &mut self,
        destination: AggregateLocation,
        source: SliceLocation,
        index: SliceElementIndex,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice aggregate copy emission requires a stack frame",
            )]);
        };
        let layout_size = u32::try_from(layout.size)
            .map_err(|_error| aggregate_copy_diagnostic("aggregate size exceeds u32 range"))?;
        if layout_size == 0 {
            return Err(aggregate_copy_diagnostic(
                "slice aggregate copy requires a non-empty aggregate layout",
            ));
        }
        validate_aggregate_copy_destination_exact(destination, 0, layout_size, frame)?;

        self.emit_scalar_spills(frame)?;
        self.emit_checked_slice_aggregate_element_address_to_x(
            source,
            index,
            layout_size,
            XReg::X9,
        )?;

        let mut chunk_offset = 0_u32;
        while chunk_offset < layout_size {
            let remaining = layout_size
                .checked_sub(chunk_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            self.emit_aggregate_copy_memory_chunk_to_scratch(XReg::X9, chunk_offset, chunk_bytes)?;
            self.emit_aggregate_copy_scratch_to_destination(
                destination,
                chunk_offset,
                chunk_bytes,
                frame,
            )?;
            chunk_offset = chunk_offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }

        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_copy_aggregate_to_slice_element(
        &mut self,
        destination: SliceLocation,
        index: SliceElementIndex,
        source: AggregateLocation,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "aggregate slice element copy emission requires a stack frame",
            )]);
        };
        let layout_size = u32::try_from(layout.size)
            .map_err(|_error| aggregate_copy_diagnostic("aggregate size exceeds u32 range"))?;
        if layout_size == 0 {
            return Err(aggregate_copy_diagnostic(
                "aggregate slice element copy requires a non-empty aggregate layout",
            ));
        }
        let source = self.aggregate_copy_source(source, layout_size, frame)?;
        validate_aggregate_copy_source_exact(source, 0, layout_size)?;

        self.emit_scalar_spills(frame)?;
        self.emit_checked_slice_aggregate_element_address_to_x(
            destination,
            index,
            layout_size,
            XReg::X9,
        )?;

        let mut chunk_offset = 0_u32;
        while chunk_offset < layout_size {
            let remaining = layout_size
                .checked_sub(chunk_offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            self.emit_aggregate_copy_source_chunk_to_scratch(source, chunk_offset, chunk_bytes)?;
            self.emit_aggregate_copy_scratch_to_memory_chunk(XReg::X9, chunk_offset, chunk_bytes)?;
            chunk_offset = chunk_offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }

        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen::values) fn emit_checked_slice_aggregate_element_address_to_x(
        &mut self,
        source: SliceLocation,
        index: SliceElementIndex,
        element_size: u32,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_slice_element_index_to_x(index, XReg::X16)?;
        match source {
            SliceLocation::Parameter(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("parameter slice length word index overflows")
                })?;
                self.emit_parameter_word_to_x(len_word_index, XReg::X8)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                self.emit_parameter_word_to_x(ptr_word_index, XReg::X8)?;
            }
            SliceLocation::Local(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("local slice length word index overflows")
                })?;
                self.emit_local_word_to_x(len_word_index, XReg::X8)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                self.emit_local_word_to_x(ptr_word_index, XReg::X8)?;
            }
            SliceLocation::Return => {
                let (ptr, len) = self.slice_location_registers(source)?;
                if len != XReg::X8 {
                    self.encoder.emit_mov_x(XReg::X8, len);
                }
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                if ptr != XReg::X8 {
                    self.encoder.emit_mov_x(XReg::X8, ptr);
                }
            }
        }

        if element_size != 1 {
            emit_mov_u64_to_x(&mut self.encoder, XReg::X17, u64::from(element_size));
            self.encoder.emit_mul_x(XReg::X16, XReg::X16, XReg::X17);
        }
        self.encoder.emit_adds_x(destination, XReg::X8, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_store_u8_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        value: &U8Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer byte store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_u8_value_to_w(value, WReg::W2)?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_i32_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        value: &I32Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer i32 store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_i32_value_to_w(value, WReg::W2)?;
        self.encoder.emit_str_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_usize_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer usize store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_usize_value_to_x(value, XReg::X2)?;
        self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_bool_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        value: &BoolValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer bool store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_bool_value_to_w(value, WReg::W2)?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_str_to_pointer(
        &mut self,
        pointer: &UsizeValue,
        offset: &UsizeValue,
        value: &StrValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "pointer str store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(pointer, XReg::X0)?;
        self.emit_usize_value_to_x(offset, XReg::X1)?;
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);
        self.emit_str_value_to_x_pair(value, XReg::X2, XReg::X3)?;
        self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0);
        self.encoder.emit_str_x_imm(XReg::X3, XReg::X0, 8);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_u8_to_slice_index(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        value: &U8Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice byte index store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_u8_value_to_w(value, WReg::W2)?;
        self.emit_checked_slice_store_address(destination, index, 0)?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_i32_to_slice_index(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        value: &I32Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice i32 index store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_i32_value_to_w(value, WReg::W2)?;
        self.emit_checked_slice_store_address(destination, index, 2)?;
        self.encoder.emit_str_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_usize_to_slice_index(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        value: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice usize index store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_usize_value_to_x(value, XReg::X2)?;
        self.emit_checked_slice_store_address(destination, index, 3)?;
        self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_bool_to_slice_index(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        value: &BoolValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice bool index store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_bool_value_to_w(value, WReg::W2)?;
        self.emit_checked_slice_store_address(destination, index, 0)?;
        self.encoder.emit_strb_w_imm(WReg::W2, XReg::X0, 0);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_store_str_to_slice_index(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        value: &StrValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "slice str index store emission requires a stack frame",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        self.emit_str_value_to_x_pair(value, XReg::X2, XReg::X3)?;
        self.emit_checked_slice_store_address(destination, index, 4)?;
        self.encoder.emit_str_x_imm(XReg::X2, XReg::X0, 0);
        self.encoder.emit_str_x_imm(XReg::X3, XReg::X0, 8);
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen::values) fn emit_checked_slice_store_address(
        &mut self,
        destination: SliceLocation,
        index: &UsizeValue,
        element_shift: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_slice_value_to_x_pair(&SliceValue::Location(destination), XReg::X0, XReg::X1)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X1)?;
        if element_shift != 0 {
            self.encoder
                .emit_lsl_x_imm(XReg::X16, XReg::X16, element_shift);
        }
        self.encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_checked_slice_element_address_to_x(
        &mut self,
        source: SliceLocation,
        index: SliceElementIndex,
        element: SliceElementAddressKind,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_slice_element_index_to_x(index, XReg::X16)?;
        match source {
            SliceLocation::Parameter(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("parameter slice length word index overflows")
                })?;
                self.emit_parameter_word_to_x(len_word_index, XReg::X8)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                self.emit_parameter_word_to_x(ptr_word_index, XReg::X8)?;
            }
            SliceLocation::Local(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("local slice length word index overflows")
                })?;
                self.emit_local_word_to_x(len_word_index, XReg::X8)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                self.emit_local_word_to_x(ptr_word_index, XReg::X8)?;
            }
            SliceLocation::Return => {
                let (ptr, len) = self.slice_location_registers(source)?;
                if len != XReg::X8 {
                    self.encoder.emit_mov_x(XReg::X8, len);
                }
                self.emit_index_in_bounds_check(XReg::X16, XReg::X8)?;
                if ptr != XReg::X8 {
                    self.encoder.emit_mov_x(XReg::X8, ptr);
                }
            }
        }
        if let Some(shift) = slice_element_address_shift(element) {
            self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, shift);
        }
        self.encoder.emit_adds_x(destination, XReg::X8, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_slice_element_index_to_x(
        &mut self,
        index: SliceElementIndex,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match index {
            SliceElementIndex::Const(value) => {
                emit_mov_u64_to_x(&mut self.encoder, destination, value);
                Ok(())
            }
            SliceElementIndex::Location(location) => {
                self.emit_usize_value_to_x(&UsizeValue::Location(location), destination)
            }
        }
    }
}
