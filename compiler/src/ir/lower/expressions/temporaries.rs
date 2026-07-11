use super::super::context::LoweringContext;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, I32Location, I32Value, Instruction, StrLocation, StrValue, UsizeLocation,
    UsizeValue,
};

pub(super) struct LoweredI32Value {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: I32Value,
}

pub(super) struct LoweredUsizeValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: UsizeValue,
}

pub(super) struct LoweredStrValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: StrValue,
}

pub(super) struct TemporaryAllocator {
    next_index: usize,
}

impl TemporaryAllocator {
    pub(super) fn new(context: &LoweringContext) -> Result<Self, Vec<Diagnostic>> {
        Ok(Self {
            next_index: context.first_temporary_local_index()?,
        })
    }

    pub(super) fn next_i32(&mut self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(I32Location::Local)
    }

    pub(super) fn next_usize(&mut self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(UsizeLocation::Local)
    }

    pub(super) fn next_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(BoolLocation::Local)
    }

    pub(super) fn next_str(&mut self) -> Result<StrLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(StrLocation::Local)
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
