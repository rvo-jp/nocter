use super::super::context::LoweringContext;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, I32Location, I32Value, Instruction, SliceLocation, SliceValue, StrLocation,
    StrValue, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use std::cell::Cell;
use std::rc::Rc;

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
    aggregate_slot_counter: Rc<Cell<usize>>,
}

impl TemporaryAllocator {
    pub(in crate::ir::lower) fn new(context: &LoweringContext) -> Result<Self, Vec<Diagnostic>> {
        Ok(Self {
            next_index: context.first_temporary_local_index()?,
            aggregate_slot_counter: context.aggregate_slot_counter(),
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
        let slot_index = self.aggregate_slot_counter.get();
        self.aggregate_slot_counter.set(slot_index + 1);
        slot_index
    }

    pub(in crate::ir::lower) fn reserved_local_abi_words(
        &self,
        context: &LoweringContext,
    ) -> Result<usize, Vec<Diagnostic>> {
        let start = context.first_temporary_local_index()?;
        Ok(self.next_index - start)
    }

    fn next_local_index(&mut self, word_count: usize) -> Result<usize, Vec<Diagnostic>> {
        let next_index = self.next_index.checked_add(word_count).ok_or_else(|| {
            vec![Diagnostic::error(
                "E8008",
                "temporary local ABI word count overflows host usize",
            )]
        })?;

        let index = self.next_index;
        self.next_index = next_index;
        Ok(index)
    }
}
