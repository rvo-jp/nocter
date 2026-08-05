use crate::abi::{ReturnPassing, ValueLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, BorrowSource, Function, I32Location, I32Value,
    Instruction, OutcomeFailureMode, ScalarArgument, SliceElementIndex, SliceLocation, SliceValue,
    StrLocation, StrValue, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use std::collections::BTreeSet;

mod aggregate_slot_requests;
mod call_arguments;
mod frame_requirements;
mod parameter_spills;
mod scalar_locals;

use aggregate_slot_requests::*;
use call_arguments::*;
use frame_requirements::*;
use parameter_spills::*;
use scalar_locals::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FunctionFrame {
    Frameless,
    Framed(FrameLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrameLayout {
    frame_size: u32,
    saved_x30_offset: u32,
    parameter_spill_slots: Vec<ParameterSpillSlot>,
    scalar_spill_slots: Vec<ScalarSpillSlot>,
    argument_staging_slots: Vec<ArgumentStagingSlot>,
    aggregate_slots: Vec<AggregateSlot>,
    indirect_return_pointer_offset: Option<u32>,
}

impl FrameLayout {
    pub(super) fn frame_size(&self) -> u32 {
        self.frame_size
    }

    pub(super) fn saved_x30_offset(&self) -> u32 {
        self.saved_x30_offset
    }

    pub(super) fn parameter_spill_slots(&self) -> &[ParameterSpillSlot] {
        &self.parameter_spill_slots
    }

    pub(super) fn parameter_spill_slot(
        &self,
        parameter_index: usize,
    ) -> Option<ParameterSpillSlot> {
        self.parameter_spill_slots
            .iter()
            .copied()
            .find(|slot| slot.parameter_index == parameter_index)
    }

    pub(super) fn scalar_spill_slots(&self) -> &[ScalarSpillSlot] {
        &self.scalar_spill_slots
    }

    pub(super) fn scalar_spill_slot(&self, local_index: usize) -> Option<ScalarSpillSlot> {
        self.scalar_spill_slots
            .iter()
            .copied()
            .find(|slot| slot.local_index == local_index)
    }

    pub(super) fn argument_staging_slots(&self) -> &[ArgumentStagingSlot] {
        &self.argument_staging_slots
    }

    #[allow(dead_code)]
    pub(super) fn aggregate_slots(&self) -> &[AggregateSlot] {
        &self.aggregate_slots
    }

    #[allow(dead_code)]
    pub(super) fn aggregate_slot(&self, slot_index: usize) -> Option<AggregateSlot> {
        self.aggregate_slots
            .iter()
            .copied()
            .find(|slot| slot.slot_index == slot_index)
    }

    pub(super) fn indirect_return_pointer_offset(&self) -> Option<u32> {
        self.indirect_return_pointer_offset
    }

    pub(super) fn for_slot_counts(
        scalar_spill_count: usize,
        argument_staging_count: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::for_slot_counts_with_parameter_spills_and_aggregate_slots(
            scalar_spill_count,
            argument_staging_count,
            &[],
            &[],
            false,
        )
    }

    #[allow(dead_code)]
    pub(super) fn for_slot_counts_with_aggregate_slots(
        scalar_spill_count: usize,
        argument_staging_count: usize,
        aggregate_slot_requests: &[AggregateSlotRequest],
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::for_slot_counts_with_parameter_spills_and_aggregate_slots(
            scalar_spill_count,
            argument_staging_count,
            &[],
            aggregate_slot_requests,
            false,
        )
    }

    fn for_slot_counts_with_parameter_spills_and_aggregate_slots(
        scalar_spill_count: usize,
        argument_staging_count: usize,
        parameter_spill_requests: &[usize],
        aggregate_slot_requests: &[AggregateSlotRequest],
        spill_indirect_return_pointer: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut parameter_spill_requests = parameter_spill_requests.to_vec();
        parameter_spill_requests.sort_unstable();
        parameter_spill_requests.dedup();
        let indirect_return_pointer_bytes = if spill_indirect_return_pointer {
            SCALAR_SLOT_SIZE
        } else {
            0
        };
        let parameter_spill_bytes = parameter_spill_requests
            .len()
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("parameter spill slot count overflows host usize")
            })?;
        let scalar_spill_bytes = scalar_spill_count
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("scalar spill slot count overflows host usize")
            })?;
        let argument_staging_bytes = argument_staging_count
            .checked_mul(SCALAR_SLOT_SIZE)
            .ok_or_else(|| {
                frame_too_large_diagnostic("argument staging slot count overflows host usize")
            })?;
        let scalar_slot_bytes = indirect_return_pointer_bytes
            .checked_add(parameter_spill_bytes)
            .and_then(|bytes| bytes.checked_add(scalar_spill_bytes))
            .and_then(|bytes| bytes.checked_add(argument_staging_bytes))
            .ok_or_else(|| frame_too_large_diagnostic("scalar slot bytes overflow host usize"))?;
        let (aggregate_slots, aggregate_slot_bytes) =
            aggregate_slots(aggregate_slot_requests, scalar_slot_bytes)?;
        let slot_bytes = scalar_slot_bytes
            .checked_add(aggregate_slot_bytes)
            .ok_or_else(|| frame_too_large_diagnostic("frame slot bytes overflow host usize"))?;
        let unaligned_frame_size = slot_bytes
            .checked_add(SAVED_X30_SLOT_SIZE)
            .ok_or_else(|| frame_too_large_diagnostic("frame size overflows host usize"))?;
        let frame_size = align_usize(unaligned_frame_size, STACK_ALIGNMENT);

        if frame_size > ADD_SUB_SP_IMM_MAX as usize {
            return Err(frame_too_large_diagnostic(
                "frame size exceeds ARM64 add/sub immediate range",
            ));
        }

        let saved_x30_offset = frame_size - SAVED_X30_SLOT_SIZE;
        if saved_x30_offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
            return Err(frame_too_large_diagnostic(
                "saved x30 slot exceeds ARM64 x-register load/store immediate range",
            ));
        }

        let indirect_return_pointer_offset = if spill_indirect_return_pointer {
            Some(0)
        } else {
            None
        };

        let mut parameter_spill_slots = Vec::with_capacity(parameter_spill_requests.len());
        for (slot_index, parameter_index) in parameter_spill_requests.into_iter().enumerate() {
            let offset = slot_index
                .checked_mul(SCALAR_SLOT_SIZE)
                .and_then(|offset| indirect_return_pointer_bytes.checked_add(offset))
                .ok_or_else(|| {
                    frame_too_large_diagnostic("parameter spill slot offset overflows host usize")
                })?;
            if offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "parameter spill slot exceeds ARM64 x-register load/store immediate range",
                ));
            }
            parameter_spill_slots.push(ParameterSpillSlot {
                parameter_index,
                offset: offset as u32,
            });
        }

        let mut scalar_spill_slots = Vec::with_capacity(scalar_spill_count);
        for local_index in 0..scalar_spill_count {
            let offset = indirect_return_pointer_bytes
                .checked_add(parameter_spill_bytes)
                .and_then(|bytes| {
                    local_index
                        .checked_mul(SCALAR_SLOT_SIZE)
                        .and_then(|offset| bytes.checked_add(offset))
                })
                .ok_or_else(|| {
                    frame_too_large_diagnostic("scalar spill slot offset overflows host usize")
                })?;
            if offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "scalar spill slot exceeds ARM64 x-register load/store immediate range",
                ));
            }
            scalar_spill_slots.push(ScalarSpillSlot {
                local_index,
                offset: offset as u32,
            });
        }

        let mut argument_staging_slots = Vec::with_capacity(argument_staging_count);
        for argument_index in 0..argument_staging_count {
            let offset = indirect_return_pointer_bytes
                .checked_add(parameter_spill_bytes)
                .and_then(|bytes| bytes.checked_add(scalar_spill_bytes))
                .and_then(|bytes| {
                    argument_index
                        .checked_mul(SCALAR_SLOT_SIZE)
                        .and_then(|offset| bytes.checked_add(offset))
                })
                .ok_or_else(|| {
                    frame_too_large_diagnostic("argument staging slot offset overflows host usize")
                })?;
            if offset > LDR_STR_W_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "argument staging slot exceeds ARM64 w-register load/store immediate range",
                ));
            }
            argument_staging_slots.push(ArgumentStagingSlot {
                abi_word_index: argument_index,
                offset: offset as u32,
            });
        }

        Ok(Self {
            frame_size: frame_size as u32,
            saved_x30_offset: saved_x30_offset as u32,
            parameter_spill_slots,
            scalar_spill_slots,
            argument_staging_slots,
            aggregate_slots,
            indirect_return_pointer_offset,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParameterSpillSlot {
    parameter_index: usize,
    offset: u32,
}

impl ParameterSpillSlot {
    pub(super) fn parameter_index(self) -> usize {
        self.parameter_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarSpillSlot {
    local_index: usize,
    offset: u32,
}

impl ScalarSpillSlot {
    pub(super) fn local_index(self) -> usize {
        self.local_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArgumentStagingSlot {
    abi_word_index: usize,
    offset: u32,
}

impl ArgumentStagingSlot {
    pub(super) fn abi_word_index(self) -> usize {
        self.abi_word_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AggregateSlotRequest {
    slot_index: usize,
    layout: ValueLayout,
}

#[allow(dead_code)]
impl AggregateSlotRequest {
    pub(super) fn new(slot_index: usize, layout: ValueLayout) -> Self {
        Self { slot_index, layout }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AggregateSlot {
    slot_index: usize,
    offset: u32,
    size: u32,
    align: u32,
}

#[allow(dead_code)]
impl AggregateSlot {
    pub(super) fn slot_index(self) -> usize {
        self.slot_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }

    pub(super) fn size(self) -> u32 {
        self.size
    }

    pub(super) fn align(self) -> u32 {
        self.align
    }
}

pub(super) fn plan_function_frame(function: &Function) -> Result<FunctionFrame, Vec<Diagnostic>> {
    let aggregate_slot_requests = aggregate_slot_requests(&function.instructions)?;
    let parameter_spill_requests = parameter_spill_requests(
        &function.instructions,
        function_clobbers_parameter_registers(&function.instructions),
    );
    let scalar_spill_count = scalar_spill_slot_count(&function.instructions);
    let requires_frame = function_requires_frame(&function.instructions);
    let has_frame = requires_frame
        || !aggregate_slot_requests.is_empty()
        || !parameter_spill_requests.is_empty()
        || scalar_spill_count > REGISTER_LOCAL_ABI_WORDS;
    let spill_indirect_return_pointer = has_frame
        && function.return_type.success_return_passing() == Some(ReturnPassing::IndirectPointer);

    if !has_frame {
        return Ok(FunctionFrame::Frameless);
    }

    let argument_staging_count = max_call_argument_count(&function.instructions);

    if !spill_indirect_return_pointer
        && aggregate_slot_requests.is_empty()
        && parameter_spill_requests.is_empty()
    {
        return FrameLayout::for_slot_counts(scalar_spill_count, argument_staging_count)
            .map(FunctionFrame::Framed);
    }

    FrameLayout::for_slot_counts_with_parameter_spills_and_aggregate_slots(
        scalar_spill_count,
        argument_staging_count,
        &parameter_spill_requests,
        &aggregate_slot_requests,
        spill_indirect_return_pointer,
    )
    .map(FunctionFrame::Framed)
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn frame_too_large_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("function stack frame is too large for the native backend: {reason}"),
    )]
}

const STACK_ALIGNMENT: usize = 16;
const SCALAR_SLOT_SIZE: usize = 8;
const SAVED_X30_SLOT_SIZE: usize = 8;
const REGISTER_LOCAL_ABI_WORDS: usize = 7;
const ADD_SUB_SP_IMM_MAX: u32 = 0x00ff_f000;
const LDR_STR_W_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 4;
const LDR_STR_X_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 8;

#[cfg(test)]
mod tests;
