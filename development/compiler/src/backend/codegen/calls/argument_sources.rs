use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_borrow_source_address_to_x(
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
            BorrowSource::I32(I32Location::Parameter(index))
            | BorrowSource::U8(U8Location::Parameter(index))
            | BorrowSource::Usize(UsizeLocation::Parameter(index))
            | BorrowSource::Bool(BoolLocation::Parameter(index)) => frame
                .parameter_spill_slot(index)
                .map(|slot| slot.offset())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument source parameter {index} has no spill slot"),
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
            BorrowSource::AggregateSlotField { slot_index, offset } => {
                let slot = frame.aggregate_slot(slot_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        format!("borrow argument aggregate slot {slot_index} is not reserved"),
                    )]
                })?;
                slot.offset().checked_add(offset).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9005",
                        "borrow argument aggregate slot field offset overflows",
                    )]
                })?
            }
            BorrowSource::AggregateParameter(index) => {
                self.emit_parameter_word_to_x(index, register)?;
                return Ok(());
            }
            BorrowSource::BorrowParameter(index) => {
                self.emit_parameter_word_to_x(index, register)?;
                return Ok(());
            }
            BorrowSource::AggregateParameterField {
                parameter_index,
                offset,
            } => {
                self.emit_parameter_word_to_x(parameter_index, register)?;
                if offset != 0 {
                    self.encoder.emit_add_x_imm(register, register, offset);
                }
                return Ok(());
            }
            BorrowSource::SliceIndex {
                source,
                index,
                element,
            } => {
                return self
                    .emit_checked_slice_element_address_to_x(source, index, element, register);
            }
            BorrowSource::PointerOffset {
                pointer,
                offset,
                field_offset,
            } => {
                self.emit_usize_value_to_x(&UsizeValue::Location(pointer), register)?;
                let scratch = if register == XReg::X16 {
                    XReg::X17
                } else {
                    XReg::X16
                };
                self.emit_usize_value_to_x(&UsizeValue::Location(offset), scratch)?;
                self.encoder.emit_add_x(register, register, scratch);
                if field_offset != 0 {
                    self.encoder
                        .emit_add_x_imm(register, register, field_offset);
                }
                return Ok(());
            }
            BorrowSource::I32(I32Location::Return)
            | BorrowSource::U8(U8Location::Return)
            | BorrowSource::Usize(UsizeLocation::Return)
            | BorrowSource::Bool(BoolLocation::Return) => {
                return Err(vec![Diagnostic::error(
                    "E9005",
                    "borrow argument emission requires a local source",
                )]);
            }
        };

        self.encoder.emit_add_x_sp_imm(register, offset);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_aggregate_argument_source_address_to_x(
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

    pub(in crate::backend::codegen) fn emit_direct_aggregate_argument_word_to_staging_slot(
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
        self.emit_aggregate_copy_stack_chunk_to_scratch(source_offset, chunk_bytes)?;
        self.encoder.emit_str_x_sp(XReg::X16, staging_slot.offset());
        Ok(())
    }
}
