use super::{
    EntryEmitter, I32_BIT_WIDTH, USIZE_BIT_WIDTH, emit_mov_i32_to_w, emit_mov_u32_to_w,
    emit_mov_u64_to_x,
};
use crate::abi::ValueLayout;
use crate::backend::frame::{AggregateSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolValue, I32Location, I32Value,
    SliceElementAddressKind, SliceElementIndex, SliceLocation, SliceValue, StrLocation, StrValue,
    U8Location, U8Value, UsizeLocation, UsizeValue,
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

mod aggregate_copy;
mod aggregate_fields;
mod arithmetic;
mod materialize;
mod memory;
mod registers;
mod setters;

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

fn slice_element_address_shift(element: SliceElementAddressKind) -> Option<u32> {
    match element {
        SliceElementAddressKind::U8 | SliceElementAddressKind::Bool => None,
        SliceElementAddressKind::I32 => Some(2),
        SliceElementAddressKind::Usize => Some(3),
        SliceElementAddressKind::Str => Some(4),
        SliceElementAddressKind::Aggregate { stride } if stride.is_power_of_two() => {
            Some(stride.trailing_zeros())
        }
        SliceElementAddressKind::Aggregate { .. } => None,
    }
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
        XReg::X8 | XReg::X19 | XReg::X20 | XReg::X21 | XReg::X22 | XReg::X23 | XReg::X30 => None,
    }
}

const AGGREGATE_USIZE_STORE_BYTES: u32 = 8;
const AGGREGATE_I32_STORE_BYTES: u32 = 4;
const AGGREGATE_U16_STORE_BYTES: u32 = 2;
const AGGREGATE_U8_STORE_BYTES: u32 = 1;
const DIRECT_AGGREGATE_RETURN_BYTES: u32 = AGGREGATE_USIZE_STORE_BYTES * 2;
