use super::{
    DARWIN_SYSCALL_TRAP, EntryEmitter, FunctionCallPatch, FunctionSymbol,
    control_flow::BranchPatch, emit_mov_i32_to_w, emit_mov_u64_to_x, values::LocalScalarWidth,
};
use crate::abi::{ABI_WORD_SIZE, ARGUMENT_REGISTER_COUNT, ValueLayout};
use crate::backend::frame::{ArgumentStagingSlot, FrameLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgumentSource, AggregateLocation, BoolLocation, BorrowSource, I32Location,
    OutcomeFailureMode, ScalarArgument, SliceLocation, StrLocation, Type, U8Location,
    UsizeLocation, UsizeValue,
};
use crate::target::arm64::{BranchCondition, WReg, XReg};

pub(super) struct OutcomeDirectAggregateCall<'a> {
    pub(super) destination: AggregateLocation,
    pub(super) function: FunctionSymbol,
    pub(super) arguments: &'a [ScalarArgument],
    pub(super) layout: ValueLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::codegen) struct OutgoingStackArguments {
    area_size: u32,
}

mod aggregate_calls;
mod argument_sources;
mod argument_staging;
mod call_sites;
mod result_locations;
mod syscalls;

fn staging_slot(
    frame: &FrameLayout,
    abi_word_index: usize,
) -> Result<ArgumentStagingSlot, Vec<Diagnostic>> {
    let slot = frame
        .argument_staging_slots()
        .get(abi_word_index)
        .copied()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E9003",
                format!("argument staging slot {abi_word_index} is not reserved"),
            )]
        })?;
    debug_assert_eq!(slot.abi_word_index(), abi_word_index);
    Ok(slot)
}

fn next_abi_word_index(index: usize, subject: &str) -> Result<usize, Vec<Diagnostic>> {
    advance_abi_word_index(index, 1, subject)
}

fn advance_abi_word_index(
    index: usize,
    words: usize,
    subject: &str,
) -> Result<usize, Vec<Diagnostic>> {
    index.checked_add(words).ok_or_else(|| {
        vec![Diagnostic::error(
            "E9003",
            format!("{subject} ABI word index overflows"),
        )]
    })
}

fn call_argument_abi_word_count(arguments: &[ScalarArgument]) -> usize {
    arguments.iter().map(ScalarArgument::abi_word_count).sum()
}

fn tail_call_has_borrow_argument(arguments: &[ScalarArgument]) -> bool {
    arguments
        .iter()
        .any(|argument| matches!(argument, ScalarArgument::Borrow(_)))
}

fn outgoing_stack_argument_area_size(abi_word_count: usize) -> Result<u32, Vec<Diagnostic>> {
    let Some(stack_words) = abi_word_count.checked_sub(ARGUMENT_REGISTER_COUNT) else {
        return Ok(0);
    };
    let bytes = stack_words
        .checked_mul(ABI_WORD_SIZE as usize)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument byte count overflows"))?;
    let aligned = align_usize(bytes, 16)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument alignment overflows"))?;
    u32::try_from(aligned)
        .map_err(|_error| outgoing_stack_argument_diagnostic("stack argument area exceeds u32"))
}

fn staged_argument_slot_offset(
    slot: ArgumentStagingSlot,
    stack_area_size: u32,
) -> Result<u32, Vec<Diagnostic>> {
    slot.offset()
        .checked_add(stack_area_size)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("staged argument offset overflows"))
}

fn outgoing_stack_argument_word_offset(abi_word_index: usize) -> Result<u32, Vec<Diagnostic>> {
    let stack_word_index = abi_word_index
        .checked_sub(ARGUMENT_REGISTER_COUNT)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("argument word is register-passed"))?;
    let offset = stack_word_index
        .checked_mul(ABI_WORD_SIZE as usize)
        .ok_or_else(|| outgoing_stack_argument_diagnostic("stack argument offset overflows"))?;
    u32::try_from(offset)
        .map_err(|_error| outgoing_stack_argument_diagnostic("stack argument offset exceeds u32"))
}

fn align_usize(value: usize, align: usize) -> Option<usize> {
    let mask = align.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

fn outgoing_stack_argument_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9003",
        format!("stack argument emission is invalid: {reason}"),
    )]
}

fn syscall_result_store_offset_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        "syscall result store offset overflows",
    )]
}

fn validate_direct_aggregate_register_layout(
    layout: ValueLayout,
    subject: &str,
) -> Result<(), Vec<Diagnostic>> {
    if layout.size > 16 {
        return Err(direct_aggregate_diagnostic(
            subject,
            "value exceeds two ABI words",
        ));
    }

    let layout_size = u32::try_from(layout.size)
        .map_err(|_error| direct_aggregate_diagnostic(subject, "size exceeds u32 range"))?;
    let mut offset = 0_u32;
    while offset < layout_size {
        let remaining_bytes = layout_size
            .checked_sub(offset)
            .ok_or_else(|| direct_aggregate_diagnostic(subject, "offset exceeds layout size"))?;
        let chunk_bytes = direct_aggregate_chunk_bytes(remaining_bytes, subject)?;
        offset = offset
            .checked_add(chunk_bytes)
            .ok_or_else(|| direct_aggregate_diagnostic(subject, "offset overflows"))?;
    }
    Ok(())
}

fn direct_aggregate_chunk_bytes(
    remaining_bytes: u32,
    subject: &str,
) -> Result<u32, Vec<Diagnostic>> {
    match remaining_bytes {
        0 => Err(unsupported_direct_aggregate_chunk_diagnostic(
            remaining_bytes,
            subject,
        )),
        1..=DIRECT_AGGREGATE_WORD_BYTES => Ok(remaining_bytes),
        _ => Ok(DIRECT_AGGREGATE_WORD_BYTES),
    }
}

fn unsupported_direct_aggregate_chunk_diagnostic(
    chunk_bytes: u32,
    subject: &str,
) -> Vec<Diagnostic> {
    direct_aggregate_diagnostic(
        subject,
        &format!("partial ABI word size {chunk_bytes} is not supported"),
    )
}

fn direct_aggregate_result_diagnostic(reason: &str) -> Vec<Diagnostic> {
    direct_aggregate_diagnostic("direct aggregate result", reason)
}

fn direct_aggregate_diagnostic(subject: &str, reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("{subject} is invalid: {reason}"),
    )]
}

const DIRECT_AGGREGATE_WORD_BYTES: u32 = 8;
