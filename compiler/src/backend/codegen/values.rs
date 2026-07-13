use super::{EntryEmitter, I32_BIT_WIDTH, USIZE_BIT_WIDTH, emit_mov_i32_to_w, emit_mov_u64_to_x};
use crate::abi::ValueLayout;
use crate::backend::frame::{AggregateSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, I32Location, I32Value, SliceLocation, SliceValue,
    StrLocation, StrValue, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::target::arm64::{BranchCondition, WReg, XReg};

#[derive(Clone, Copy)]
enum AggregateCopySource {
    Slot(AggregateSlot),
    Parameter(XReg),
    DirectParameter { start_index: usize },
}

impl EntryEmitter {
    pub(super) fn emit_set_i32(
        &mut self,
        destination: I32Location,
        value: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(value, destination)
    }

    pub(super) fn emit_set_usize(
        &mut self,
        destination: UsizeLocation,
        value: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(value, destination)
    }

    pub(super) fn emit_store_aggregate_usize(
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
                self.encoder.emit_str_x_imm(XReg::X16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => self.emit_x_to_direct_aggregate_return(offset),
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_str_x_imm(XReg::X16, base, offset);
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

    pub(super) fn emit_store_aggregate_i32(
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
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_str_w_imm(WReg::W16, base, offset);
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

    pub(super) fn emit_store_aggregate_u8(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &U8Value,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_u8_value_to_w(value, WReg::W16)?;
        self.emit_store_aggregate_byte(destination, offset, frame)
    }

    pub(super) fn emit_store_aggregate_bool(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        value: &BoolValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_bool_value_to_w(value, WReg::W16)?;
        self.emit_store_aggregate_byte(destination, offset, frame)
    }

    fn emit_store_aggregate_byte(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => {
                self.encoder.emit_strb_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_strb_w_imm(WReg::W16, base, offset);
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

    pub(super) fn emit_load_aggregate_usize(
        &mut self,
        destination: UsizeLocation,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_usize_field_offset(offset)?;
        let destination = self.usize_location_register(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_USIZE_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_ldr_x_sp(destination, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_ldr_x_imm(destination, base, offset);
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
            AggregateLocation::DirectParameter { .. } => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from direct parameter registers",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn emit_load_aggregate_i32(
        &mut self,
        destination: I32Location,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_aggregate_i32_field_offset(offset)?;
        let destination = self.i32_location_register(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_I32_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_ldr_w_sp(destination, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_ldr_w_imm(destination, base, offset);
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
            AggregateLocation::DirectParameter { .. } => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from direct parameter registers",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn emit_load_aggregate_u8(
        &mut self,
        destination: U8Location,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.u8_location_register(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_ldrb_w_sp(destination, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_ldrb_w_imm(destination, base, offset);
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
            AggregateLocation::DirectParameter { .. } => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from direct parameter registers",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn emit_load_aggregate_bool(
        &mut self,
        destination: BoolLocation,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.bool_location_register(destination)?;
        match source {
            AggregateLocation::Slot(_) => {
                let source_offset = self.aggregate_slot_load_offset(
                    source,
                    offset,
                    AGGREGATE_U8_STORE_BYTES,
                    frame,
                )?;
                self.encoder.emit_ldrb_w_sp(destination, source_offset);
            }
            AggregateLocation::Parameter(index) => {
                let base = aggregate_parameter_register(index)?;
                self.encoder.emit_ldrb_w_imm(destination, base, offset);
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from return locations",
                ));
            }
            AggregateLocation::DirectParameter { .. } => {
                return Err(aggregate_load_diagnostic(
                    "aggregate field load cannot read from direct parameter registers",
                ));
            }
        }
        Ok(())
    }

    fn aggregate_slot_load_offset(
        &self,
        source: AggregateLocation,
        offset: u32,
        load_bytes: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<u32, Vec<Diagnostic>> {
        let AggregateLocation::Slot(slot_index) = source else {
            return Err(aggregate_load_diagnostic(
                "backend v0 can only load aggregate fields from slots",
            ));
        };
        self.aggregate_slot_field_offset(slot_index, offset, load_bytes, frame)
    }

    fn aggregate_slot_field_offset(
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

    pub(super) fn emit_copy_aggregate(
        &mut self,
        destination: AggregateLocation,
        source: AggregateLocation,
        layout: ValueLayout,
        frame: Option<&FrameLayout>,
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
        validate_aggregate_copy_destination(destination, layout_size, frame)?;

        let mut offset = 0_u32;
        while offset < layout_size {
            let remaining = layout_size
                .checked_sub(offset)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset exceeds aggregate size"))?;
            let chunk_bytes = aggregate_copy_chunk_bytes(remaining)?;
            self.emit_aggregate_copy_source_chunk_to_scratch(source, offset, chunk_bytes)?;
            self.emit_aggregate_copy_scratch_to_destination(
                destination,
                offset,
                chunk_bytes,
                frame,
            )?;
            offset = offset
                .checked_add(chunk_bytes)
                .ok_or_else(|| aggregate_copy_diagnostic("copy offset overflows"))?;
        }

        Ok(())
    }

    fn aggregate_copy_source(
        &self,
        source: AggregateLocation,
        layout_size: u32,
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
                if source_slot.size() != layout_size {
                    return Err(aggregate_copy_diagnostic(
                        "source slot size does not match aggregate layout",
                    ));
                }
                Ok(AggregateCopySource::Slot(source_slot))
            }
            AggregateLocation::Parameter(index) => {
                let register = XReg::argument(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!(
                            "aggregate parameter copy source word {index} has no argument register"
                        ),
                    )]
                })?;
                Ok(AggregateCopySource::Parameter(register))
            }
            AggregateLocation::DirectParameter { start_index } => {
                Ok(AggregateCopySource::DirectParameter { start_index })
            }
            AggregateLocation::Return | AggregateLocation::DirectReturn => Err(
                aggregate_copy_diagnostic("aggregate copy cannot read from return locations"),
            ),
        }
    }

    fn emit_aggregate_copy_source_chunk_to_scratch(
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
            AggregateCopySource::DirectParameter { start_index } => {
                let word_index = usize::try_from(offset / AGGREGATE_USIZE_STORE_BYTES)
                    .map_err(|_error| aggregate_copy_diagnostic("copy word index overflows"))?;
                let register_index = start_index
                    .checked_add(word_index)
                    .ok_or_else(|| aggregate_copy_diagnostic("copy word index overflows"))?;
                let source_register = XReg::argument(register_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!(
                            "direct aggregate parameter copy source word {register_index} has no argument register"
                        ),
                    )]
                })?;
                match chunk_bytes {
                    AGGREGATE_USIZE_STORE_BYTES => {
                        if source_register != XReg::X16 {
                            self.encoder.emit_mov_x(XReg::X16, source_register);
                        }
                    }
                    AGGREGATE_I32_STORE_BYTES | AGGREGATE_U8_STORE_BYTES => {
                        let source_register = WReg::argument(register_index).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "E9005",
                                format!(
                                    "direct aggregate parameter copy source word {register_index} has no argument register"
                                ),
                            )]
                        })?;
                        if source_register != WReg::W16 {
                            self.encoder.emit_mov_w(WReg::W16, source_register);
                        }
                    }
                    _ => return Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
                }
            }
        }
        Ok(())
    }

    fn emit_aggregate_copy_scratch_to_destination(
        &mut self,
        destination: AggregateLocation,
        offset: u32,
        chunk_bytes: u32,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            AggregateLocation::Return => {
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
            AggregateLocation::Parameter(_) | AggregateLocation::DirectParameter { .. } => Err(
                aggregate_copy_diagnostic("aggregate copy cannot target parameter locations"),
            ),
        }
    }

    fn emit_aggregate_copy_stack_chunk_to_scratch(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_ldr_x_sp(XReg::X16, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_ldr_w_sp(WReg::W16, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_ldrb_w_sp(WReg::W16, offset),
            _ => return Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
        }
        Ok(())
    }

    fn emit_aggregate_copy_memory_chunk_to_scratch(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_ldr_x_imm(XReg::X16, base, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_ldr_w_imm(WReg::W16, base, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_ldrb_w_imm(WReg::W16, base, offset),
            _ => return Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
        }
        Ok(())
    }

    fn emit_aggregate_copy_scratch_to_stack_chunk(
        &mut self,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_str_x_sp(XReg::X16, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_str_w_sp(WReg::W16, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_strb_w_sp(WReg::W16, offset),
            _ => return Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
        }
        Ok(())
    }

    fn emit_aggregate_copy_scratch_to_memory_chunk(
        &mut self,
        base: XReg,
        offset: u32,
        chunk_bytes: u32,
    ) -> Result<(), Vec<Diagnostic>> {
        match chunk_bytes {
            AGGREGATE_USIZE_STORE_BYTES => self.encoder.emit_str_x_imm(XReg::X16, base, offset),
            AGGREGATE_I32_STORE_BYTES => self.encoder.emit_str_w_imm(WReg::W16, base, offset),
            AGGREGATE_U8_STORE_BYTES => self.encoder.emit_strb_w_imm(WReg::W16, base, offset),
            _ => return Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
        }
        Ok(())
    }

    fn emit_x_to_direct_aggregate_return(&mut self, offset: u32) -> Result<(), Vec<Diagnostic>> {
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

    pub(super) fn emit_set_u8(
        &mut self,
        destination: U8Location,
        value: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.u8_location_register(destination)?;
        self.emit_u8_value_to_w(value, destination)
    }

    pub(super) fn emit_set_bool(
        &mut self,
        destination: BoolLocation,
        value: &BoolValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.bool_location_register(destination)?;
        self.emit_bool_value_to_w(value, destination)
    }

    pub(super) fn emit_set_str(
        &mut self,
        destination: StrLocation,
        value: &StrValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let (ptr_destination, len_destination) = self.str_location_registers(destination)?;
        self.emit_str_value_to_x_pair(value, ptr_destination, len_destination)
    }

    pub(super) fn emit_set_slice(
        &mut self,
        destination: SliceLocation,
        value: &SliceValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let (ptr_destination, len_destination) = self.slice_location_registers(destination)?;
        self.emit_slice_value_to_x_pair(value, ptr_destination, len_destination)
    }

    pub(super) fn emit_add_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_adds_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 addition non-overflow target")?;
        Ok(())
    }

    pub(super) fn emit_subtract_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder
            .emit_subs_w(destination, WReg::W16, destination);
        self.emit_i32_overflow_check("i32 subtraction non-overflow target")?;
        Ok(())
    }

    pub(super) fn emit_multiply_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.encoder.emit_smull_x(XReg::X17, WReg::W16, destination);
        self.encoder.emit_sxtw_x_w(XReg::X16, WReg::W17);
        self.encoder.emit_cmp_x(XReg::X17, XReg::X16);
        let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(exact_fit, "i32 multiplication exact-fit target")?;
        if destination != WReg::W17 {
            self.encoder.emit_mov_w(destination, WReg::W17);
        }
        Ok(())
    }

    pub(super) fn emit_divide_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder
            .emit_sdiv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(super) fn emit_remainder_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_division_safety_checks(WReg::W16, destination)?;
        self.encoder.emit_sdiv_w(WReg::W17, WReg::W16, destination);
        self.encoder
            .emit_msub_w(destination, WReg::W17, destination, WReg::W16);
        Ok(())
    }

    pub(super) fn emit_shift_left_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(super) fn emit_shift_right_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.i32_location_register(destination)?;
        self.emit_i32_value_to_w(left, WReg::W16)?;
        self.emit_i32_value_to_w(right, destination)?;
        self.emit_i32_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_asrv_w(destination, WReg::W16, destination);
        Ok(())
    }

    pub(super) fn emit_add_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder
            .emit_adds_x(destination, XReg::X16, destination);
        self.emit_usize_no_carry_check("usize addition non-overflow target")?;
        Ok(())
    }

    pub(super) fn emit_subtract_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder
            .emit_subs_x(destination, XReg::X16, destination);
        self.emit_usize_no_borrow_check("usize subtraction non-underflow target")?;
        Ok(())
    }

    pub(super) fn emit_multiply_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.encoder.emit_umulh_x(XReg::X17, XReg::X16, destination);
        self.encoder.emit_cmp_x_zero(XReg::X17);
        let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            exact_fit,
            "usize multiplication exact-fit target",
        )?;
        self.encoder.emit_mul_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(super) fn emit_divide_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_division_safety_checks(destination)?;
        self.encoder
            .emit_udiv_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(super) fn emit_remainder_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_division_safety_checks(destination)?;
        self.encoder.emit_udiv_x(XReg::X17, XReg::X16, destination);
        self.encoder
            .emit_msub_x(destination, XReg::X17, destination, XReg::X16);
        Ok(())
    }

    pub(super) fn emit_shift_left_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lslv_x(destination, XReg::X16, destination);
        Ok(())
    }

    pub(super) fn emit_shift_right_usize(
        &mut self,
        destination: UsizeLocation,
        left: &UsizeValue,
        right: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination = self.usize_location_register(destination)?;
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lsrv_x(destination, XReg::X16, destination);
        Ok(())
    }

    fn emit_i32_shift_count_safety_checks(&mut self, count: WReg) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(count);
        let count_nonnegative = self.emit_cond_branch_placeholder(BranchCondition::Ge);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_nonnegative, "shift non-negative target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, I32_BIT_WIDTH);
        self.encoder.emit_cmp_w(count, WReg::W17);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Lt);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    fn emit_usize_shift_count_safety_checks(&mut self, count: XReg) -> Result<(), Vec<Diagnostic>> {
        emit_mov_u64_to_x(&mut self.encoder, XReg::X17, USIZE_BIT_WIDTH);
        self.encoder.emit_cmp_x(count, XReg::X17);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    fn emit_usize_division_safety_checks(&mut self, divisor: XReg) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x_zero(divisor);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        Ok(())
    }

    fn emit_i32_division_safety_checks(
        &mut self,
        dividend: WReg,
        divisor: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(divisor);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, i32::MIN);
        self.encoder.emit_cmp_w(dividend, WReg::W17);
        let dividend_not_min = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, -1);
        self.encoder.emit_cmp_w(divisor, WReg::W17);
        let divisor_not_minus_one = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            dividend_not_min,
            "signed division overflow dividend target",
        )?;
        self.patch_branch_placeholder_to_current(
            divisor_not_minus_one,
            "signed division overflow divisor target",
        )?;

        Ok(())
    }

    fn emit_usize_no_carry_check(
        &mut self,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let no_carry = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_carry, target_description)?;
        Ok(())
    }

    fn emit_usize_no_borrow_check(
        &mut self,
        target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let no_borrow = self.emit_cond_branch_placeholder(BranchCondition::Cs);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_borrow, target_description)?;
        Ok(())
    }

    fn emit_i32_overflow_check(&mut self, target_description: &str) -> Result<(), Vec<Diagnostic>> {
        let no_overflow = self.emit_cond_branch_placeholder(BranchCondition::Vc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(no_overflow, target_description)?;
        Ok(())
    }

    pub(super) fn emit_trap(&mut self) {
        self.encoder.emit_brk(0);
    }

    pub(super) fn emit_i32_value_to_w(
        &mut self,
        value: &I32Value,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            I32Value::Const(value) => emit_mov_i32_to_w(&mut self.encoder, destination, *value),
            I32Value::Location(location) => {
                let source = self.i32_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_w(destination, source);
                }
            }
            I32Value::U8ZeroExtend(value) => {
                self.emit_u8_value_to_w(value, destination)?;
            }
        }

        Ok(())
    }

    pub(super) fn emit_usize_value_to_x(
        &mut self,
        value: &UsizeValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            UsizeValue::Const(value) => emit_mov_u64_to_x(&mut self.encoder, destination, *value),
            UsizeValue::Location(location) => {
                let source = self.usize_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_x(destination, source);
                }
            }
            UsizeValue::U8ZeroExtend(value) => {
                self.emit_u8_value_to_w(value, WReg::W16)?;
                if destination != XReg::X16 {
                    self.encoder.emit_mov_x(destination, XReg::X16);
                }
            }
            UsizeValue::StrLen(location) => {
                let (_, source) = self.str_location_registers(*location)?;
                if source != destination {
                    self.encoder.emit_mov_x(destination, source);
                }
            }
            UsizeValue::SliceLen(location) => {
                let (_, source) = self.slice_location_registers(*location)?;
                if source != destination {
                    self.encoder.emit_mov_x(destination, source);
                }
            }
        }

        Ok(())
    }

    pub(super) fn emit_u8_value_to_w(
        &mut self,
        value: &U8Value,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            U8Value::Const(value) => {
                emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(*value));
            }
            U8Value::Location(location) => {
                let source = self.u8_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_w(destination, source);
                }
            }
            U8Value::StrIndex { source, index } => {
                let (ptr, len) = self.str_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
            U8Value::StaticStrIndex { bytes, index } => {
                self.emit_usize_value_to_x(index, XReg::X16)?;
                emit_mov_u64_to_x(&mut self.encoder, XReg::X17, bytes.len() as u64);
                self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
                self.emit_static_data_address(XReg::X17, bytes);
                self.encoder
                    .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
            }
            U8Value::SliceIndex { source, index } => {
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
        }

        Ok(())
    }

    fn emit_checked_byte_load(
        &mut self,
        destination: WReg,
        ptr: XReg,
        len: XReg,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_index_in_bounds_check(XReg::X16, len)?;
        self.encoder.emit_ldrb_w_reg(destination, ptr, XReg::X16);
        Ok(())
    }

    fn emit_index_in_bounds_check(
        &mut self,
        index: XReg,
        len: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x(index, len);
        let in_bounds = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(in_bounds, "index in-bounds target")?;
        Ok(())
    }

    pub(super) fn emit_bool_value_to_w(
        &mut self,
        value: &BoolValue,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            BoolValue::Const(value) => {
                emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(*value));
            }
            BoolValue::Location(location) => {
                let source = self.bool_location_register(*location)?;
                if source != destination {
                    self.encoder.emit_mov_w(destination, source);
                }
            }
            BoolValue::Not(_)
            | BoolValue::Logical { .. }
            | BoolValue::I32Comparison { .. }
            | BoolValue::UsizeComparison { .. }
            | BoolValue::BoolComparison { .. } => {
                let branches_to_false = self.emit_bool_false_branch_placeholders(value)?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 1);
                let branch_to_end = self.emit_branch_placeholder();
                self.patch_branch_placeholders_to_current(
                    branches_to_false,
                    "bool false materialization target",
                )?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 0);
                self.patch_branch_placeholder_to_current(
                    branch_to_end,
                    "bool materialization end target",
                )?;
            }
        }

        Ok(())
    }

    pub(super) fn emit_str_value_to_x_pair(
        &mut self,
        value: &StrValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                self.emit_static_data_address(ptr_destination, bytes);
                emit_mov_u64_to_x(&mut self.encoder, len_destination, bytes.len() as u64);
            }
            StrValue::Location(location) => {
                let (ptr_source, len_source) = self.str_location_registers(*location)?;
                if ptr_source != ptr_destination {
                    self.encoder.emit_mov_x(ptr_destination, ptr_source);
                }
                if len_source != len_destination {
                    self.encoder.emit_mov_x(len_destination, len_source);
                }
            }
        }

        Ok(())
    }

    pub(super) fn emit_slice_value_to_x_pair(
        &mut self,
        value: &SliceValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            SliceValue::Location(location) => {
                let (ptr_source, len_source) = self.slice_location_registers(*location)?;
                if ptr_source != ptr_destination {
                    self.encoder.emit_mov_x(ptr_destination, ptr_source);
                }
                if len_source != len_destination {
                    self.encoder.emit_mov_x(len_destination, len_source);
                }
            }
        }

        Ok(())
    }
}

fn validate_aggregate_usize_field_offset(offset: u32) -> Result<(), Vec<Diagnostic>> {
    if !offset.is_multiple_of(AGGREGATE_USIZE_STORE_BYTES) {
        return Err(aggregate_store_offset_diagnostic(
            "usize field offset is not 8-byte aligned",
        ));
    }

    Ok(())
}

fn validate_aggregate_i32_field_offset(offset: u32) -> Result<(), Vec<Diagnostic>> {
    if !offset.is_multiple_of(AGGREGATE_I32_STORE_BYTES) {
        return Err(aggregate_store_offset_diagnostic(
            "i32 field offset is not 4-byte aligned",
        ));
    }

    Ok(())
}

fn aggregate_parameter_register(index: usize) -> Result<XReg, Vec<Diagnostic>> {
    XReg::argument(index).ok_or_else(|| {
        vec![Diagnostic::error(
            "E9005",
            format!("aggregate parameter pointer word {index} has no argument register"),
        )]
    })
}

fn aggregate_store_offset_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("aggregate field store offset is invalid: {reason}"),
    )]
}

fn aggregate_load_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("aggregate field load is invalid: {reason}"),
    )]
}

fn aggregate_copy_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("aggregate copy is invalid: {reason}"),
    )]
}

fn aggregate_copy_chunk_bytes(remaining_bytes: u32) -> Result<u32, Vec<Diagnostic>> {
    if remaining_bytes >= AGGREGATE_USIZE_STORE_BYTES {
        return Ok(AGGREGATE_USIZE_STORE_BYTES);
    }
    match remaining_bytes {
        AGGREGATE_I32_STORE_BYTES | AGGREGATE_U8_STORE_BYTES => Ok(remaining_bytes),
        _ => Err(unsupported_aggregate_copy_chunk_diagnostic(remaining_bytes)),
    }
}

fn validate_aggregate_copy_destination(
    destination: AggregateLocation,
    layout_size: u32,
    frame: &FrameLayout,
) -> Result<(), Vec<Diagnostic>> {
    if let AggregateLocation::Slot(destination_slot_index) = destination {
        let destination_slot = frame
            .aggregate_slot(destination_slot_index)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9005",
                    format!(
                        "aggregate copy destination slot {destination_slot_index} is not reserved"
                    ),
                )]
            })?;
        if destination_slot.size() != layout_size {
            return Err(aggregate_copy_diagnostic(
                "destination slot size does not match aggregate layout",
            ));
        }
    }
    Ok(())
}

fn unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes: u32) -> Vec<Diagnostic> {
    aggregate_copy_diagnostic(&format!(
        "partial ABI word size {chunk_bytes} is not supported"
    ))
}

const AGGREGATE_USIZE_STORE_BYTES: u32 = 8;
const AGGREGATE_I32_STORE_BYTES: u32 = 4;
const AGGREGATE_U8_STORE_BYTES: u32 = 1;
