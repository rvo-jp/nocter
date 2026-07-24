use super::{
    EntryEmitter, I32_BIT_WIDTH, USIZE_BIT_WIDTH, emit_mov_i32_to_w, emit_mov_u32_to_w,
    emit_mov_u64_to_x,
};
use crate::abi::ValueLayout;
use crate::backend::frame::{AggregateSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolValue, I32Location, I32Value,
    SliceLocation, SliceValue, StrLocation, StrValue, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::target::arm64::{BranchCondition, MoveWideShift, WReg, XReg};

#[derive(Clone, Copy)]
enum AggregateCopySource {
    Slot(AggregateSlot),
    Parameter(XReg),
    StackParameterPointer { parameter_index: usize },
    DirectParameter { start_index: usize },
}

#[derive(Clone, Copy)]
pub(super) enum LocalScalarWidth {
    I32,
    Byte,
}

impl EntryEmitter {
    fn scalar_local_offset(&self, local_index: usize) -> Result<u32, Vec<Diagnostic>> {
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

    fn emit_local_word_to_x(
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

    fn emit_local_word_to_w(
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

    pub(super) fn emit_x_to_local_word(
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

    pub(super) fn emit_w_to_local_word(
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

    fn emit_local_word_pair_to_x_pair(
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

    fn emit_parameter_word_pair_to_x_pair(
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

    pub(super) fn emit_x_pair_to_local_words(
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

    pub(super) fn emit_x_pair_to_x_pair(
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

    fn i32_register_destination(
        &self,
        destination: I32Location,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            I32Location::Local(index) => Ok(WReg::local(index)),
            _ => self.i32_location_register(destination).map(Some),
        }
    }

    fn usize_register_destination(
        &self,
        destination: UsizeLocation,
    ) -> Result<Option<XReg>, Vec<Diagnostic>> {
        match destination {
            UsizeLocation::Local(index) => Ok(XReg::local(index)),
            _ => self.usize_location_register(destination).map(Some),
        }
    }

    fn u8_register_destination(
        &self,
        destination: U8Location,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            U8Location::Local(index) => Ok(WReg::local(index)),
            _ => self.u8_location_register(destination).map(Some),
        }
    }

    fn bool_register_destination(
        &self,
        destination: BoolLocation,
    ) -> Result<Option<WReg>, Vec<Diagnostic>> {
        match destination {
            BoolLocation::Local(index) => Ok(WReg::local(index)),
            _ => self.bool_location_register(destination).map(Some),
        }
    }

    fn i32_register_destination_or_scratch(
        &self,
        destination: I32Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .i32_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    fn usize_register_destination_or_scratch(
        &self,
        destination: UsizeLocation,
    ) -> Result<XReg, Vec<Diagnostic>> {
        Ok(self
            .usize_register_destination(destination)?
            .unwrap_or(XReg::X16))
    }

    fn u8_register_destination_or_scratch(
        &self,
        destination: U8Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .u8_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    fn bool_register_destination_or_scratch(
        &self,
        destination: BoolLocation,
    ) -> Result<WReg, Vec<Diagnostic>> {
        Ok(self
            .bool_register_destination(destination)?
            .unwrap_or(WReg::W16))
    }

    pub(super) fn emit_set_i32(
        &mut self,
        destination: I32Location,
        value: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.i32_register_destination_or_scratch(destination)?;
        self.emit_i32_value_to_w(value, destination_register)?;
        self.emit_w_to_i32_location(destination_register, destination)
    }

    pub(super) fn emit_set_usize(
        &mut self,
        destination: UsizeLocation,
        value: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.usize_register_destination_or_scratch(destination)?;
        self.emit_usize_value_to_x(value, destination_register)?;
        self.emit_x_to_usize_location(destination_register, destination)
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
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_str_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
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

    pub(super) fn emit_store_aggregate_u32(
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
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
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

    pub(super) fn emit_store_aggregate_u16(
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
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
                self.encoder.emit_strh_w_imm(WReg::W16, base, offset);
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
                self.emit_indirect_return_pointer_to_x8(frame);
                self.encoder.emit_strb_w_imm(WReg::W16, XReg::X8, offset);
                Ok(())
            }
            AggregateLocation::DirectReturn => Err(aggregate_store_offset_diagnostic(
                "direct aggregate return field store must be an 8-byte word",
            )),
            AggregateLocation::Parameter(index) => {
                let base = self.aggregate_parameter_base_register(index)?;
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

    pub(super) fn emit_load_aggregate_i32(
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

    pub(super) fn emit_load_aggregate_u8(
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

    pub(super) fn emit_load_aggregate_bool(
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
        self.emit_copy_aggregate_range_checked(destination, 0, source, 0, layout, frame, true)
    }

    pub(super) fn emit_copy_aggregate_range(
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

    fn emit_copy_aggregate_range_checked(
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

    fn aggregate_copy_source(
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
            AggregateLocation::DirectParameter { .. } => Err(aggregate_copy_diagnostic(
                "aggregate copy cannot target direct parameter locations",
            )),
        }
    }

    pub(super) fn emit_aggregate_copy_stack_chunk_to_scratch(
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

    pub(super) fn emit_aggregate_copy_memory_chunk_to_scratch(
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

    pub(super) fn emit_aggregate_copy_scratch_to_stack_chunk(
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

    pub(super) fn emit_aggregate_copy_scratch_to_memory_chunk(
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

    pub(super) fn emit_aggregate_copy_x_to_stack_chunk(
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

    fn emit_aggregate_copy_stack_bytes_to_scratch(
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

    fn emit_aggregate_copy_memory_bytes_to_scratch(
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

    fn emit_aggregate_copy_byte_to_scratch(&mut self, byte_offset: u32) {
        if byte_offset != 0 {
            self.encoder
                .emit_lsl_x_imm(XReg::X17, XReg::X17, byte_offset * 8);
        }
        self.encoder.emit_orr_x(XReg::X16, XReg::X16, XReg::X17);
    }

    fn emit_aggregate_copy_scratch_to_stack_bytes(
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

    fn emit_aggregate_copy_scratch_to_memory_bytes(
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

    fn emit_aggregate_copy_scratch_byte_to_w17(&mut self, byte_offset: u32) {
        if byte_offset == 0 {
            self.encoder.emit_mov_w(WReg::W17, WReg::W16);
        } else {
            self.encoder
                .emit_lsr_x_imm(XReg::X17, XReg::X16, byte_offset * 8);
        }
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

    fn aggregate_parameter_base_register(&mut self, index: usize) -> Result<XReg, Vec<Diagnostic>> {
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

    fn emit_direct_aggregate_parameter_chunk_to_w(
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

    fn emit_direct_aggregate_parameter_chunk_to_scratch(
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

    pub(super) fn emit_set_u8(
        &mut self,
        destination: U8Location,
        value: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.u8_register_destination_or_scratch(destination)?;
        self.emit_u8_value_to_w(value, destination_register)?;
        self.emit_w_to_u8_location(destination_register, destination)
    }

    pub(super) fn emit_set_bool(
        &mut self,
        destination: BoolLocation,
        value: &BoolValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.bool_register_destination_or_scratch(destination)?;
        self.emit_bool_value_to_w(value, destination_register)?;
        self.emit_w_to_bool_location(destination_register, destination)
    }

    pub(super) fn emit_set_str(
        &mut self,
        destination: StrLocation,
        value: &StrValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_str_value_to_x_pair(value, XReg::X16, XReg::X17)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(super) fn emit_set_str_raw_parts(
        &mut self,
        destination: StrLocation,
        pointer: &UsizeValue,
        len: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(len, XReg::X17)?;
        self.emit_usize_value_to_x(pointer, XReg::X16)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(super) fn emit_set_slice(
        &mut self,
        destination: SliceLocation,
        value: &SliceValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_slice_value_to_x_pair(value, XReg::X16, XReg::X17)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
    }

    pub(super) fn emit_set_slice_raw_parts(
        &mut self,
        destination: SliceLocation,
        pointer: &UsizeValue,
        len: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(len, XReg::X17)?;
        self.emit_usize_value_to_x(pointer, XReg::X16)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
    }

    pub(super) fn emit_add_i32(
        &mut self,
        destination: I32Location,
        left: &I32Value,
        right: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_adds_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_i32_overflow_check("i32 addition non-overflow target")?;
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_subs_w(WReg::W16, WReg::W16, WReg::W17);
            self.emit_i32_overflow_check("i32 subtraction non-overflow target")?;
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_smull_x(XReg::X17, WReg::W16, WReg::W17);
            self.encoder.emit_sxtw_x_w(XReg::X16, WReg::W17);
            self.encoder.emit_cmp_x(XReg::X17, XReg::X16);
            let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
            self.emit_trap();
            self.patch_branch_placeholder_to_current(
                exact_fit,
                "i32 multiplication exact-fit target",
            )?;
            return self.emit_w_to_i32_location(WReg::W17, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_division_safety_checks_for_values(left, right)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_sdiv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_division_safety_checks_for_values(left, right)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.encoder.emit_sdiv_w(WReg::W8, WReg::W16, WReg::W17);
            self.encoder
                .emit_msub_w(WReg::W16, WReg::W8, WReg::W17, WReg::W16);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.emit_i32_shift_count_safety_checks_with_scratch(WReg::W17, WReg::W16)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.encoder.emit_lslv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.i32_register_destination(destination)? else {
            self.emit_i32_value_to_w(right, WReg::W17)?;
            self.emit_i32_shift_count_safety_checks_with_scratch(WReg::W17, WReg::W16)?;
            self.emit_i32_value_to_w(left, WReg::W16)?;
            self.encoder.emit_asrv_w(WReg::W16, WReg::W16, WReg::W17);
            return self.emit_w_to_i32_location(WReg::W16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.encoder.emit_adds_x(XReg::X16, XReg::X16, XReg::X17);
            self.emit_usize_no_carry_check("usize addition non-overflow target")?;
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.encoder.emit_subs_x(XReg::X16, XReg::X16, XReg::X17);
            self.emit_usize_no_borrow_check("usize subtraction non-underflow target")?;
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.emit_usize_value_to_x(right, XReg::X8)?;
            self.encoder.emit_umulh_x(XReg::X17, XReg::X16, XReg::X8);
            self.encoder.emit_cmp_x_zero(XReg::X17);
            let exact_fit = self.emit_cond_branch_placeholder(BranchCondition::Eq);
            self.emit_trap();
            self.patch_branch_placeholder_to_current(
                exact_fit,
                "usize multiplication exact-fit target",
            )?;
            self.encoder.emit_mul_x(XReg::X16, XReg::X16, XReg::X8);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_division_safety_checks(XReg::X17)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_udiv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_division_safety_checks(XReg::X17)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_udiv_x(XReg::X8, XReg::X16, XReg::X17);
            self.encoder
                .emit_msub_x(XReg::X16, XReg::X8, XReg::X17, XReg::X16);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_shift_count_safety_checks_with_scratch(XReg::X17, XReg::X16)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_lslv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
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
        let Some(destination) = self.usize_register_destination(destination)? else {
            self.emit_usize_value_to_x(right, XReg::X17)?;
            self.emit_usize_shift_count_safety_checks_with_scratch(XReg::X17, XReg::X16)?;
            self.emit_usize_value_to_x(left, XReg::X16)?;
            self.encoder.emit_lsrv_x(XReg::X16, XReg::X16, XReg::X17);
            return self.emit_x_to_usize_location(XReg::X16, destination);
        };
        self.emit_usize_value_to_x(left, XReg::X16)?;
        self.emit_usize_value_to_x(right, destination)?;
        self.emit_usize_shift_count_safety_checks(destination)?;
        self.encoder
            .emit_lsrv_x(destination, XReg::X16, destination);
        Ok(())
    }

    fn emit_i32_shift_count_safety_checks(&mut self, count: WReg) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_shift_count_safety_checks_with_scratch(count, WReg::W17)
    }

    fn emit_i32_shift_count_safety_checks_with_scratch(
        &mut self,
        count: WReg,
        scratch: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_w_zero(count);
        let count_nonnegative = self.emit_cond_branch_placeholder(BranchCondition::Ge);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_nonnegative, "shift non-negative target")?;

        emit_mov_i32_to_w(&mut self.encoder, scratch, I32_BIT_WIDTH);
        self.encoder.emit_cmp_w(count, scratch);
        let count_in_range = self.emit_cond_branch_placeholder(BranchCondition::Lt);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(count_in_range, "shift count in-range target")?;

        Ok(())
    }

    fn emit_usize_shift_count_safety_checks(&mut self, count: XReg) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_shift_count_safety_checks_with_scratch(count, XReg::X17)
    }

    fn emit_usize_shift_count_safety_checks_with_scratch(
        &mut self,
        count: XReg,
        scratch: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        emit_mov_u64_to_x(&mut self.encoder, scratch, USIZE_BIT_WIDTH);
        self.encoder.emit_cmp_x(count, scratch);
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

    fn emit_i32_division_safety_checks_for_values(
        &mut self,
        dividend: &I32Value,
        divisor: &I32Value,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_i32_value_to_w(divisor, WReg::W17)?;
        self.encoder.emit_cmp_w_zero(WReg::W17);
        let divisor_nonzero = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(divisor_nonzero, "division non-zero target")?;

        emit_mov_i32_to_w(&mut self.encoder, WReg::W16, -1);
        self.encoder.emit_cmp_w(WReg::W17, WReg::W16);
        let divisor_not_minus_one = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_i32_value_to_w(dividend, WReg::W16)?;
        emit_mov_i32_to_w(&mut self.encoder, WReg::W17, i32::MIN);
        self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
        let dividend_not_min = self.emit_cond_branch_placeholder(BranchCondition::Ne);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(
            divisor_not_minus_one,
            "signed division overflow divisor target",
        )?;
        self.patch_branch_placeholder_to_current(
            dividend_not_min,
            "signed division overflow dividend target",
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

    pub(super) fn emit_copy_str_to_pointer(
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

    pub(super) fn emit_copy_pointer_bytes(
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

    pub(super) fn emit_store_u8_to_pointer(
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

    pub(super) fn emit_store_i32_to_pointer(
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

    pub(super) fn emit_store_usize_to_pointer(
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

    pub(super) fn emit_store_bool_to_pointer(
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

    pub(super) fn emit_store_str_to_pointer(
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
                if let I32Location::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let I32Location::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::I32)?;
                } else {
                    let source = self.i32_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
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
                if let UsizeLocation::Parameter(index) = location {
                    self.emit_parameter_word_to_x(*index, destination)?;
                    return Ok(());
                }
                if let UsizeLocation::Local(index) = location {
                    self.emit_local_word_to_x(*index, destination)?;
                } else {
                    let source = self.usize_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::U8ZeroExtend(value) => {
                self.emit_u8_value_to_w(value, WReg::W16)?;
                if destination != XReg::X16 {
                    self.encoder.emit_mov_x(destination, XReg::X16);
                }
            }
            UsizeValue::StrLen(location) => {
                if let StrLocation::Parameter(index) = *location {
                    let len_index = pair_len_index(index, "parameter str")?;
                    self.emit_parameter_word_to_x(len_index, destination)?;
                    return Ok(());
                }
                if let StrLocation::Local(index) = *location {
                    let len_index = pair_len_index(index, "local str")?;
                    self.emit_local_word_to_x(len_index, destination)?;
                } else {
                    let (_, source) = self.str_location_registers(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::SliceLen(location) => {
                if let SliceLocation::Parameter(index) = *location {
                    let len_index = pair_len_index(index, "parameter slice")?;
                    self.emit_parameter_word_to_x(len_index, destination)?;
                    return Ok(());
                }
                if let SliceLocation::Local(index) = *location {
                    let len_index = pair_len_index(index, "local slice")?;
                    self.emit_local_word_to_x(len_index, destination)?;
                } else {
                    let (_, source) = self.slice_location_registers(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::SliceIndex { source, index } => {
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_usize_load(
                        destination,
                        parameter_index,
                        index.as_ref(),
                    )?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_usize_load(destination, local_index, index.as_ref())?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_usize_load(destination, ptr, len, index.as_ref())?;
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
                if let U8Location::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let U8Location::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::Byte)?;
                } else {
                    let source = self.u8_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
                }
            }
            U8Value::StrIndex { source, index } => {
                if let StrLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_byte_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let StrLocation::Local(local_index) = *source {
                    self.emit_checked_local_byte_load(destination, local_index, index)?;
                    return Ok(());
                }
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
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_byte_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_byte_load(destination, local_index, index)?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
        }

        Ok(())
    }

    fn emit_checked_parameter_byte_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("parameter length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
        self.encoder
            .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
        Ok(())
    }

    fn emit_checked_local_byte_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("local length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_local_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
        self.encoder
            .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
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

    fn emit_checked_parameter_usize_load(
        &mut self,
        destination: XReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("parameter length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_usize_load(destination, XReg::X17);
        Ok(())
    }

    fn emit_checked_local_usize_load(
        &mut self,
        destination: XReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("local length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_local_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_usize_load(destination, XReg::X17);
        Ok(())
    }

    fn emit_checked_usize_load(
        &mut self,
        destination: XReg,
        ptr: XReg,
        len: XReg,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_index_in_bounds_check(XReg::X16, len)?;
        self.emit_indexed_usize_load(destination, ptr);
        Ok(())
    }

    fn emit_indexed_usize_load(&mut self, destination: XReg, ptr: XReg) {
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 3);
        self.encoder.emit_adds_x(XReg::X17, ptr, XReg::X16);
        self.encoder.emit_ldr_x_imm(destination, XReg::X17, 0);
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
                if let BoolLocation::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let BoolLocation::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::Byte)?;
                } else {
                    let source = self.bool_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
                }
            }
            BoolValue::StrComparison {
                operator,
                left,
                right,
            } => self.emit_str_comparison_to_w(*operator, left, right, destination)?,
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

    fn emit_str_comparison_to_w(
        &mut self,
        operator: BoolComparisonOperator,
        left: &StrValue,
        right: &StrValue,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let (equal_result, not_equal_result) = match operator {
            BoolComparisonOperator::Equal => (true, false),
            BoolComparisonOperator::NotEqual => (false, true),
        };

        self.emit_str_len_to_x(left, XReg::X16)?;
        self.emit_str_len_to_x(right, XReg::X17)?;
        self.encoder.emit_cmp_x(XReg::X16, XReg::X17);
        let mut not_equal_branches = vec![self.emit_cond_branch_placeholder(BranchCondition::Ne)];

        self.encoder.emit_cmp_x_zero(XReg::X16);
        let equal_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);

        let loop_start = self.encoder.position();
        self.emit_str_pointer_to_x(left, XReg::X16)?;
        self.encoder.emit_ldrb_w_reg(WReg::W16, XReg::X16, XReg::X8);
        self.emit_str_pointer_to_x(right, XReg::X17)?;
        self.encoder.emit_ldrb_w_reg(WReg::W17, XReg::X17, XReg::X8);
        self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
        not_equal_branches.push(self.emit_cond_branch_placeholder(BranchCondition::Ne));

        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        self.emit_str_len_to_x(left, XReg::X16)?;
        self.encoder.emit_cmp_x(XReg::X8, XReg::X16);
        let has_more = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.patch_branch_placeholder_to_offset(
            has_more,
            loop_start,
            "string comparison loop target",
        )?;

        self.patch_branch_placeholder_to_current(equal_branch, "empty string equality target")?;
        emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(equal_result));
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholders_to_current(not_equal_branches, "string inequality target")?;
        emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(not_equal_result));
        self.patch_branch_placeholder_to_current(end_branch, "string comparison end target")
    }

    fn emit_str_pointer_to_x(
        &mut self,
        value: &StrValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                self.emit_static_data_address(destination, bytes);
            }
            StrValue::Location(StrLocation::Return) => {
                if destination != XReg::X0 {
                    self.encoder.emit_mov_x(destination, XReg::X0);
                }
            }
            StrValue::Location(StrLocation::Parameter(index)) => {
                self.emit_parameter_word_to_x(*index, destination)?;
            }
            StrValue::Location(StrLocation::Local(index)) => {
                self.emit_local_word_to_x(*index, destination)?;
            }
        }
        Ok(())
    }

    fn emit_str_len_to_x(
        &mut self,
        value: &StrValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                emit_mov_u64_to_x(&mut self.encoder, destination, bytes.len() as u64);
            }
            StrValue::Location(StrLocation::Return) => {
                if destination != XReg::X1 {
                    self.encoder.emit_mov_x(destination, XReg::X1);
                }
            }
            StrValue::Location(StrLocation::Parameter(index)) => {
                let len_index = pair_len_index(*index, "parameter str")?;
                self.emit_parameter_word_to_x(len_index, destination)?;
            }
            StrValue::Location(StrLocation::Local(index)) => {
                let len_index = pair_len_index(*index, "local str")?;
                self.emit_local_word_to_x(len_index, destination)?;
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
                if let StrLocation::Parameter(index) = *location {
                    self.emit_parameter_word_pair_to_x_pair(
                        index,
                        ptr_destination,
                        len_destination,
                    )?;
                    return Ok(());
                }
                if let StrLocation::Local(index) = *location {
                    self.emit_local_word_pair_to_x_pair(index, ptr_destination, len_destination)?;
                } else {
                    let (ptr_source, len_source) = self.str_location_registers(*location)?;
                    self.emit_x_pair_to_x_pair(
                        ptr_source,
                        len_source,
                        ptr_destination,
                        len_destination,
                    )?;
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
            SliceValue::StrBytes(text) => {
                self.emit_str_value_to_x_pair(text, ptr_destination, len_destination)?;
            }
            SliceValue::Location(location) => {
                if let SliceLocation::Parameter(index) = *location {
                    self.emit_parameter_word_pair_to_x_pair(
                        index,
                        ptr_destination,
                        len_destination,
                    )?;
                    return Ok(());
                }
                if let SliceLocation::Local(index) = *location {
                    self.emit_local_word_pair_to_x_pair(index, ptr_destination, len_destination)?;
                } else {
                    let (ptr_source, len_source) = self.slice_location_registers(*location)?;
                    self.emit_x_pair_to_x_pair(
                        ptr_source,
                        len_source,
                        ptr_destination,
                        len_destination,
                    )?;
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

fn pair_len_index(first_index: usize, subject: &str) -> Result<usize, Vec<Diagnostic>> {
    first_index.checked_add(1).ok_or_else(|| {
        vec![Diagnostic::error(
            "E9005",
            format!("{subject} length word index overflows"),
        )]
    })
}

fn pair_scratch_register(excluded: &[XReg]) -> Result<XReg, Vec<Diagnostic>> {
    [XReg::X17, XReg::X16, XReg::X8]
        .into_iter()
        .find(|register| !excluded.contains(register))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E9005",
                "view pair move has no available scratch register",
            )]
        })
}

fn validate_aggregate_i32_field_offset(offset: u32) -> Result<(), Vec<Diagnostic>> {
    if !offset.is_multiple_of(AGGREGATE_I32_STORE_BYTES) {
        return Err(aggregate_store_offset_diagnostic(
            "i32 field offset is not 4-byte aligned",
        ));
    }

    Ok(())
}

fn validate_aggregate_u16_field_offset(offset: u32) -> Result<(), Vec<Diagnostic>> {
    if !offset.is_multiple_of(AGGREGATE_U16_STORE_BYTES) {
        return Err(aggregate_store_offset_diagnostic(
            "u16 field offset is not 2-byte aligned",
        ));
    }

    Ok(())
}

fn direct_aggregate_parameter_word_index(
    start_index: usize,
    offset: u32,
    subject: &str,
) -> Result<usize, Vec<Diagnostic>> {
    if !offset.is_multiple_of(AGGREGATE_USIZE_STORE_BYTES) {
        return Err(direct_aggregate_parameter_load_diagnostic(
            subject,
            "offset is not 8-byte aligned",
        ));
    }

    let word_index = usize::try_from(offset / AGGREGATE_USIZE_STORE_BYTES).map_err(|_error| {
        direct_aggregate_parameter_load_diagnostic(subject, "word index overflows")
    })?;
    direct_aggregate_parameter_word_index_from_word(start_index, word_index, subject)
}

fn direct_aggregate_parameter_chunk_source(
    start_index: usize,
    offset: u32,
    chunk_bytes: u32,
    subject: &str,
) -> Result<(usize, u32), Vec<Diagnostic>> {
    validate_aggregate_copy_chunk_bytes(chunk_bytes)?;

    let byte_offset = offset % AGGREGATE_USIZE_STORE_BYTES;
    let end = byte_offset.checked_add(chunk_bytes).ok_or_else(|| {
        direct_aggregate_parameter_load_diagnostic(subject, "field range end overflows")
    })?;
    if end > AGGREGATE_USIZE_STORE_BYTES {
        return Err(direct_aggregate_parameter_load_diagnostic(
            subject,
            "field crosses an ABI word boundary",
        ));
    }

    let word_index = usize::try_from(offset / AGGREGATE_USIZE_STORE_BYTES).map_err(|_error| {
        direct_aggregate_parameter_load_diagnostic(subject, "word index overflows")
    })?;
    let word_index =
        direct_aggregate_parameter_word_index_from_word(start_index, word_index, subject)?;
    Ok((word_index, byte_offset))
}

fn direct_aggregate_parameter_word_index_from_word(
    start_index: usize,
    word_index: usize,
    subject: &str,
) -> Result<usize, Vec<Diagnostic>> {
    let register_index = start_index.checked_add(word_index).ok_or_else(|| {
        direct_aggregate_parameter_load_diagnostic(subject, "register index overflows")
    })?;
    Ok(register_index)
}

fn direct_aggregate_parameter_load_diagnostic(subject: &str, reason: &str) -> Vec<Diagnostic> {
    aggregate_load_diagnostic(&format!(
        "direct aggregate parameter {subject} is invalid: {reason}"
    ))
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

fn indexed_load_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("indexed load is invalid: {reason}"),
    )]
}

fn aggregate_copy_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("aggregate copy is invalid: {reason}"),
    )]
}

fn aggregate_copy_chunk_bytes(remaining_bytes: u32) -> Result<u32, Vec<Diagnostic>> {
    match remaining_bytes {
        0 => Err(unsupported_aggregate_copy_chunk_diagnostic(remaining_bytes)),
        1..=AGGREGATE_USIZE_STORE_BYTES => Ok(remaining_bytes),
        _ => Ok(AGGREGATE_USIZE_STORE_BYTES),
    }
}

fn validate_aggregate_copy_destination_exact(
    destination: AggregateLocation,
    destination_offset: u32,
    layout_size: u32,
    frame: &FrameLayout,
) -> Result<(), Vec<Diagnostic>> {
    validate_aggregate_copy_destination_range(destination, destination_offset, layout_size, frame)?;
    if destination_offset != 0 {
        return Err(aggregate_copy_diagnostic(
            "exact aggregate copy destination offset must be 0",
        ));
    }
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

fn validate_aggregate_copy_source_exact(
    source: AggregateCopySource,
    source_offset: u32,
    layout_size: u32,
) -> Result<(), Vec<Diagnostic>> {
    if source_offset != 0 {
        return Err(aggregate_copy_diagnostic(
            "exact aggregate copy source offset must be 0",
        ));
    }
    if let AggregateCopySource::Slot(source_slot) = source
        && source_slot.size() != layout_size
    {
        return Err(aggregate_copy_diagnostic(
            "source slot size does not match aggregate layout",
        ));
    }
    Ok(())
}

fn validate_aggregate_copy_destination_range(
    destination: AggregateLocation,
    destination_offset: u32,
    layout_size: u32,
    frame: &FrameLayout,
) -> Result<(), Vec<Diagnostic>> {
    match destination {
        AggregateLocation::Slot(destination_slot_index) => {
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
            validate_aggregate_copy_slot_range(
                destination_offset,
                layout_size,
                destination_slot.size(),
                "destination range exceeds aggregate slot size",
            )
        }
        AggregateLocation::DirectReturn => {
            if !matches!(destination_offset, 0 | AGGREGATE_USIZE_STORE_BYTES) {
                return Err(aggregate_copy_diagnostic(
                    "direct aggregate return range offset must be 0 or 8",
                ));
            }
            let range_end = destination_offset.checked_add(layout_size).ok_or_else(|| {
                aggregate_copy_diagnostic("direct aggregate return range end overflows")
            })?;
            if range_end > DIRECT_AGGREGATE_RETURN_BYTES {
                return Err(aggregate_copy_diagnostic(
                    "direct aggregate return range exceeds two ABI words",
                ));
            }
            Ok(())
        }
        AggregateLocation::Return | AggregateLocation::Parameter(_) => Ok(()),
        AggregateLocation::DirectParameter { .. } => Err(aggregate_copy_diagnostic(
            "aggregate copy cannot target direct parameter locations",
        )),
    }
}

fn validate_aggregate_copy_source_range(
    source: AggregateCopySource,
    source_offset: u32,
    layout_size: u32,
    _frame: &FrameLayout,
) -> Result<(), Vec<Diagnostic>> {
    match source {
        AggregateCopySource::Slot(source_slot) => validate_aggregate_copy_slot_range(
            source_offset,
            layout_size,
            source_slot.size(),
            "source range exceeds aggregate slot size",
        ),
        AggregateCopySource::Parameter(_)
        | AggregateCopySource::StackParameterPointer { .. }
        | AggregateCopySource::DirectParameter { .. } => Ok(()),
    }
}

fn validate_aggregate_copy_slot_range(
    offset: u32,
    layout_size: u32,
    slot_size: u32,
    reason: &str,
) -> Result<(), Vec<Diagnostic>> {
    let end = offset
        .checked_add(layout_size)
        .ok_or_else(|| aggregate_copy_diagnostic("aggregate copy range end overflows"))?;
    if end > slot_size {
        return Err(aggregate_copy_diagnostic(reason));
    }
    Ok(())
}

fn unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes: u32) -> Vec<Diagnostic> {
    aggregate_copy_diagnostic(&format!(
        "partial ABI word size {chunk_bytes} is not supported"
    ))
}

fn validate_aggregate_copy_chunk_bytes(chunk_bytes: u32) -> Result<(), Vec<Diagnostic>> {
    match chunk_bytes {
        1..=AGGREGATE_USIZE_STORE_BYTES => Ok(()),
        _ => Err(unsupported_aggregate_copy_chunk_diagnostic(chunk_bytes)),
    }
}

fn aggregate_copy_chunk_has_aligned_offset(offset: u32, chunk_bytes: u32) -> bool {
    matches!(
        chunk_bytes,
        AGGREGATE_USIZE_STORE_BYTES
            | AGGREGATE_I32_STORE_BYTES
            | AGGREGATE_U16_STORE_BYTES
            | AGGREGATE_U8_STORE_BYTES
    ) && offset.is_multiple_of(chunk_bytes)
}

fn w_reg_for_x_reg(register: XReg) -> Option<WReg> {
    match register {
        XReg::X0 => Some(WReg::W0),
        XReg::X1 => Some(WReg::W1),
        XReg::X2 => Some(WReg::W2),
        XReg::X3 => Some(WReg::W3),
        XReg::X4 => Some(WReg::W4),
        XReg::X5 => Some(WReg::W5),
        XReg::X6 => Some(WReg::W6),
        XReg::X7 => Some(WReg::W7),
        XReg::X9 => Some(WReg::W9),
        XReg::X10 => Some(WReg::W10),
        XReg::X11 => Some(WReg::W11),
        XReg::X12 => Some(WReg::W12),
        XReg::X13 => Some(WReg::W13),
        XReg::X14 => Some(WReg::W14),
        XReg::X15 => Some(WReg::W15),
        XReg::X16 => Some(WReg::W16),
        XReg::X17 => Some(WReg::W17),
        XReg::X8 | XReg::X30 => None,
    }
}

const AGGREGATE_USIZE_STORE_BYTES: u32 = 8;
const AGGREGATE_I32_STORE_BYTES: u32 = 4;
const AGGREGATE_U16_STORE_BYTES: u32 = 2;
const AGGREGATE_U8_STORE_BYTES: u32 = 1;
const DIRECT_AGGREGATE_RETURN_BYTES: u32 = AGGREGATE_USIZE_STORE_BYTES * 2;
