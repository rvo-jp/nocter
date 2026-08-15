//! Source-parameter to machine-ABI projection.
//!
//! ABI words form one tagged sequence.  A word cannot accidentally be present
//! in several type-specific side tables, and padding has an explicit identity.
//! Source parameter storage is kept separately because one source value can
//! consume several ABI words or an aggregate staging slot.

use crate::abi::{ValueClassification, ValueLayout};
use crate::ast::TypeExpr;
use crate::integer::IntegerType;
use crate::outcomes::storage::OutcomeStorageLayout;
use crate::typecheck::TypecheckSliceElementKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SliceTypeInfo {
    pub(super) element_kind: TypecheckSliceElementKind,
    pub(super) element_type: Option<TypeExpr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbiWordBinding {
    I32,
    U8,
    Usize,
    Integer(IntegerType),
    Bool,
    StrPointer,
    SlicePointer,
    ErrorCode,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParameterStorage {
    I32 {
        abi_index: usize,
    },
    U8 {
        abi_index: usize,
    },
    Usize {
        abi_index: usize,
    },
    Integer {
        kind: IntegerType,
        abi_index: usize,
    },
    Bool {
        abi_index: usize,
    },
    Str {
        abi_index: usize,
    },
    Slice {
        abi_index: usize,
    },
    Borrow {
        abi_index: usize,
    },
    Error {
        abi_index: usize,
    },
    Aggregate {
        slot_index: usize,
        layout: ValueLayout,
        classification: ValueClassification,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AggregateParameterSource {
    Indirect { parameter_index: usize },
    Direct { start_index: usize, words: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoweringAggregateParameter {
    pub(super) name: String,
    pub(super) layout: ValueLayout,
    pub(super) slot_index: usize,
    pub(super) source: AggregateParameterSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoweringOutcomeParameter {
    pub(super) name: String,
    pub(super) storage: OutcomeStorageLayout,
    pub(super) slot_index: usize,
    pub(super) source: AggregateParameterSource,
}

#[derive(Default)]
pub(super) struct LoweringParameterSlots {
    abi_words: Vec<AbiWordBinding>,
    source_storage: Vec<ParameterStorage>,
    pub(super) aggregates: Vec<LoweringAggregateParameter>,
    pub(super) outcomes: Vec<LoweringOutcomeParameter>,
}

impl LoweringParameterSlots {
    fn push_scalar(&mut self, word: AbiWordBinding, storage: fn(usize) -> ParameterStorage) {
        let abi_index = self.abi_words.len();
        self.abi_words.push(word);
        self.source_storage.push(storage(abi_index));
    }

    pub(super) fn push_i32_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::I32, |abi_index| ParameterStorage::I32 {
            abi_index,
        });
    }

    pub(super) fn push_u8_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::U8, |abi_index| ParameterStorage::U8 {
            abi_index,
        });
    }

    pub(super) fn push_usize_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::Usize, |abi_index| ParameterStorage::Usize {
            abi_index,
        });
    }

    pub(super) fn push_integer_parameter(&mut self, kind: IntegerType) {
        let abi_index = self.abi_words.len();
        self.abi_words.push(AbiWordBinding::Integer(kind));
        self.source_storage
            .push(ParameterStorage::Integer { kind, abi_index });
    }

    pub(super) fn push_bool_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::Bool, |abi_index| ParameterStorage::Bool {
            abi_index,
        });
    }

    pub(super) fn push_str_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::StrPointer, |abi_index| {
            ParameterStorage::Str { abi_index }
        });
    }

    pub(super) fn push_slice_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::SlicePointer, |abi_index| {
            ParameterStorage::Slice { abi_index }
        });
    }

    pub(super) fn push_error_parameter(&mut self) {
        self.push_scalar(AbiWordBinding::ErrorCode, |abi_index| {
            ParameterStorage::Error { abi_index }
        });
        self.reserve_empty_abi_words(3);
    }

    pub(super) fn push_empty_abi_word(&mut self) {
        self.abi_words.push(AbiWordBinding::Reserved);
    }

    pub(super) fn reserve_empty_abi_words(&mut self, words: usize) -> usize {
        let start = self.abi_words.len();
        self.abi_words
            .extend(std::iter::repeat_n(AbiWordBinding::Reserved, words));
        start
    }

    pub(super) fn parameter_abi_word_count(&self) -> usize {
        self.abi_words.len()
    }

    pub(super) fn source_storage(&self) -> &[ParameterStorage] {
        &self.source_storage
    }

    pub(super) fn push_source_storage(&mut self, storage: ParameterStorage) {
        self.source_storage.push(storage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiword_values_have_one_tagged_head_and_explicit_reserved_words() {
        let mut slots = LoweringParameterSlots::default();
        slots.push_str_parameter();
        slots.push_empty_abi_word();
        slots.push_error_parameter();

        assert_eq!(
            slots.abi_words,
            [
                AbiWordBinding::StrPointer,
                AbiWordBinding::Reserved,
                AbiWordBinding::ErrorCode,
                AbiWordBinding::Reserved,
                AbiWordBinding::Reserved,
                AbiWordBinding::Reserved,
            ]
        );
        assert_eq!(slots.parameter_abi_word_count(), 6);
    }
}
