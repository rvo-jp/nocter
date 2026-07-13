use super::super::context::LoweringContext;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, I32Location, I32Value, Instruction, SliceLocation, SliceValue, StrLocation,
    StrValue, U8Location, U8Value, UsizeLocation, UsizeValue,
};

pub(super) struct LoweredI32Value {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: I32Value,
}

pub(super) struct LoweredU8Value {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: U8Value,
}

pub(super) struct LoweredUsizeValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: UsizeValue,
}

pub(super) struct LoweredStrValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: StrValue,
}

pub(super) struct LoweredSliceValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: SliceValue,
}

pub(in crate::ir::lower) struct TemporaryAllocator {
    next_index: usize,
    next_aggregate_slot_index: usize,
}

impl TemporaryAllocator {
    pub(in crate::ir::lower) fn new(context: &LoweringContext) -> Result<Self, Vec<Diagnostic>> {
        Ok(Self {
            next_index: context.first_temporary_local_index()?,
            next_aggregate_slot_index: context.next_aggregate_slot_index(),
        })
    }

    pub(in crate::ir::lower) fn next_i32(&mut self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(I32Location::Local)
    }

    pub(in crate::ir::lower) fn next_u8(&mut self) -> Result<U8Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(U8Location::Local)
    }

    pub(in crate::ir::lower) fn next_usize(&mut self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(UsizeLocation::Local)
    }

    pub(in crate::ir::lower) fn next_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(BoolLocation::Local)
    }

    pub(in crate::ir::lower) fn next_str(&mut self) -> Result<StrLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(StrLocation::Local)
    }

    pub(in crate::ir::lower) fn next_slice(&mut self) -> Result<SliceLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(SliceLocation::Local)
    }

    pub(in crate::ir::lower) fn next_aggregate_slot(&mut self) -> usize {
        let slot_index = self.next_aggregate_slot_index;
        self.next_aggregate_slot_index += 1;
        slot_index
    }

    fn next_local_index(&mut self, word_count: usize) -> Result<usize, Vec<Diagnostic>> {
        if self.next_index + word_count > MAX_TEMPORARY_LOCAL_ABI_WORDS {
            return Err(vec![Diagnostic::error(
                "E8008",
                format!(
                    "IR v0 can only lower up to {MAX_TEMPORARY_LOCAL_ABI_WORDS} local ABI words"
                ),
            )]);
        }

        let index = self.next_index;
        self.next_index += word_count;
        Ok(index)
    }
}

const MAX_TEMPORARY_LOCAL_ABI_WORDS: usize = 7;
