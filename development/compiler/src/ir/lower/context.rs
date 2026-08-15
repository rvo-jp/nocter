//! Runtime names, call signatures, and parameter ABI storage used by MIR projection.

use crate::abi::{ReturnPassing, ValueLayout};
use crate::ast::TypeExpr;
use crate::integer::IntegerType;
use crate::ir::{CallTarget, Type};
use crate::outcomes::storage::OutcomeStorageLayout;
use crate::semantic::DefId;
use crate::typecheck::{TypecheckSliceElementKind, TypedHir};
use std::collections::HashMap;

use super::errors::ErrorPayload;

pub(super) type ErrorPayloads = HashMap<CallTarget, ErrorPayload>;
pub(super) type ResolvedSources<'a> = crate::resolve::ResolvedSources<'a>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SliceTypeInfo {
    pub(super) element_kind: TypecheckSliceElementKind,
    pub(super) element_type: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SliceBinding {
    pub(super) name: String,
    pub(super) info: SliceTypeInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BorrowParameter {
    pub(super) name: String,
    pub(super) parameter_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateBorrowParameter {
    pub(super) name: String,
    pub(super) parameter_index: usize,
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
        classification: crate::abi::ValueClassification,
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
    source_storage: Vec<ParameterStorage>,
    pub(super) i32: Vec<Option<String>>,
    pub(super) u8: Vec<Option<String>>,
    pub(super) usize: Vec<Option<String>>,
    pub(super) integer: Vec<Option<(String, IntegerType)>>,
    pub(super) bool: Vec<Option<String>>,
    pub(super) str: Vec<Option<String>>,
    pub(super) slice: Vec<Option<SliceBinding>>,
    pub(super) error: Vec<Option<String>>,
    pub(super) borrow_parameters: Vec<BorrowParameter>,
    pub(super) aggregates: Vec<LoweringAggregateParameter>,
    pub(super) outcomes: Vec<LoweringOutcomeParameter>,
    pub(super) aggregate_borrows: Vec<AggregateBorrowParameter>,
}

impl LoweringParameterSlots {
    pub(super) fn push_i32_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(Some(name), None, None, None, None, None, None);
        self.source_storage
            .push(ParameterStorage::I32 { abi_index });
    }

    pub(super) fn push_u8_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(None, Some(name), None, None, None, None, None);
        self.source_storage.push(ParameterStorage::U8 { abi_index });
    }

    pub(super) fn push_usize_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(None, None, Some(name), None, None, None, None);
        self.source_storage
            .push(ParameterStorage::Usize { abi_index });
    }

    pub(super) fn push_integer_parameter(&mut self, name: String, kind: IntegerType) {
        let index = self.next_parameter_index();
        self.push_empty_abi_word();
        self.integer[index] = Some((name, kind));
        self.source_storage.push(ParameterStorage::Integer {
            kind,
            abi_index: index,
        });
    }

    pub(super) fn push_bool_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(None, None, None, Some(name), None, None, None);
        self.source_storage
            .push(ParameterStorage::Bool { abi_index });
    }

    pub(super) fn push_str_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(None, None, None, None, Some(name), None, None);
        self.source_storage
            .push(ParameterStorage::Str { abi_index });
    }

    pub(super) fn push_slice_parameter(
        &mut self,
        name: String,
        element_kind: TypecheckSliceElementKind,
        element_type: Option<TypeExpr>,
    ) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(
            None,
            None,
            None,
            None,
            None,
            Some(SliceBinding {
                name,
                info: SliceTypeInfo {
                    element_kind,
                    element_type,
                },
            }),
            None,
        );
        self.source_storage
            .push(ParameterStorage::Slice { abi_index });
    }

    pub(super) fn push_error_parameter(&mut self, name: String) {
        let abi_index = self.next_parameter_index();
        self.push_abi_word(None, None, None, None, None, None, Some(name));
        self.reserve_empty_abi_words(3);
        self.source_storage
            .push(ParameterStorage::Error { abi_index });
    }

    pub(super) fn push_empty_abi_word(&mut self) {
        self.push_abi_word(None, None, None, None, None, None, None);
    }

    pub(super) fn reserve_empty_abi_words(&mut self, words: usize) -> usize {
        let start = self.next_parameter_index();
        for _ in 0..words {
            self.push_empty_abi_word();
        }
        start
    }

    pub(super) fn parameter_abi_word_count(&self) -> usize {
        self.i32.len()
    }

    pub(super) fn source_storage(&self) -> &[ParameterStorage] {
        &self.source_storage
    }

    pub(super) fn push_source_storage(&mut self, storage: ParameterStorage) {
        self.source_storage.push(storage);
    }

    fn next_parameter_index(&self) -> usize {
        self.i32.len()
    }

    #[allow(clippy::too_many_arguments)]
    fn push_abi_word(
        &mut self,
        i32_name: Option<String>,
        u8_name: Option<String>,
        usize_name: Option<String>,
        bool_name: Option<String>,
        str_name: Option<String>,
        slice_name: Option<SliceBinding>,
        error_name: Option<String>,
    ) {
        self.i32.push(i32_name);
        self.u8.push(u8_name);
        self.usize.push(usize_name);
        self.integer.push(None);
        self.bool.push(bool_name);
        self.str.push(str_name);
        self.slice.push(slice_name);
        self.error.push(error_name);
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionNames {
    by_definition: HashMap<DefId, CallTarget>,
    by_instance: crate::mir::MonoItemRegistry<CallTarget>,
    drops_by_definition_and_type: HashMap<(DefId, crate::semantic::TypeIdentity), CallTarget>,
}

impl FunctionNames {
    pub(super) fn from_index(
        functions: Vec<(DefId, CallTarget)>,
        instances: Vec<(crate::mir::CallInstanceKey, CallTarget)>,
        drops: Vec<(DefId, TypeExpr, CallTarget)>,
    ) -> Self {
        Self {
            by_definition: functions.into_iter().collect(),
            by_instance: crate::mir::MonoItemRegistry::from_entries(instances),
            drops_by_definition_and_type: drops
                .into_iter()
                .map(|(definition, ty, name)| {
                    (
                        (
                            definition,
                            crate::semantic::TypeIdentity::runtime_drop_subject(&ty),
                        ),
                        name,
                    )
                })
                .collect(),
        }
    }

    fn target_for_definition(&self, definition: DefId) -> Option<&CallTarget> {
        self.by_definition.get(&definition)
    }

    pub(in crate::ir::lower) fn target_for_instance(
        &self,
        instance: &crate::mir::CallInstance,
        typed_hir: &TypedHir,
    ) -> Option<&CallTarget> {
        if instance.receiver.is_none()
            && instance.type_arguments.is_empty()
            && let crate::mir::CallableIdentity::Definition(definition) = instance.callable
        {
            return self.target_for_definition(definition);
        }
        let key = crate::mir::CallInstanceKey::from_instance(instance, typed_hir);
        self.by_instance.value_for(key.as_ref()?)
    }

    pub(in crate::ir::lower) fn target_for_drop(
        &self,
        definition: DefId,
        ty: &TypeExpr,
    ) -> Option<&CallTarget> {
        self.drops_by_definition_and_type.get(&(
            definition,
            crate::semantic::TypeIdentity::runtime_drop_subject(ty),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionSignatures {
    signatures: HashMap<CallTarget, FunctionSignature>,
}

impl FunctionSignatures {
    pub(super) fn from_call_targets(signatures: HashMap<CallTarget, FunctionSignature>) -> Self {
        Self { signatures }
    }

    pub(super) fn return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.signatures
            .get(target)
            .map(|signature| &signature.return_type)
    }

    pub(super) fn success_return_passing(&self, target: &CallTarget) -> Option<ReturnPassing> {
        self.signatures
            .get(target)
            .and_then(|signature| signature.success_return_passing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FunctionSignature {
    pub(super) return_type: Type,
    pub(super) parameter_types: Option<Vec<Type>>,
    pub(super) parameter_abi_word_count: Option<usize>,
    pub(super) success_return_passing: Option<ReturnPassing>,
}
