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
        self.push_abi_word(Some(name), None, None, None, None, None, None);
    }

    pub(super) fn push_u8_parameter(&mut self, name: String) {
        self.push_abi_word(None, Some(name), None, None, None, None, None);
    }

    pub(super) fn push_usize_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, Some(name), None, None, None, None);
    }

    pub(super) fn push_integer_parameter(&mut self, name: String, kind: IntegerType) {
        let index = self.next_parameter_index();
        self.push_empty_abi_word();
        self.integer[index] = Some((name, kind));
    }

    pub(super) fn push_bool_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, Some(name), None, None, None);
    }

    pub(super) fn push_str_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, None, Some(name), None, None);
    }

    pub(super) fn push_slice_parameter(
        &mut self,
        name: String,
        element_kind: TypecheckSliceElementKind,
        element_type: Option<TypeExpr>,
    ) {
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
    }

    pub(super) fn push_error_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, None, None, None, Some(name));
        self.reserve_empty_abi_words(3);
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
    by_definition: HashMap<DefId, String>,
    by_instance: HashMap<crate::mir::CallInstanceKey, String>,
    by_unqualified_receiver_instance: HashMap<crate::mir::CallInstanceKey, Option<String>>,
    by_receiver_determined_instance: HashMap<crate::mir::CallInstanceKey, Option<String>>,
    by_unqualified_receiver_determined_instance:
        HashMap<crate::mir::CallInstanceKey, Option<String>>,
    drops_by_definition_and_type: HashMap<(DefId, String), String>,
    target_aliases: HashMap<String, CallTarget>,
}

impl FunctionNames {
    pub(super) fn from_index(
        functions: Vec<(DefId, String)>,
        instances: Vec<(crate::mir::CallInstanceKey, String)>,
        drops: Vec<(DefId, TypeExpr, String)>,
        target_aliases: Vec<(String, CallTarget)>,
    ) -> Self {
        let mut by_unqualified_receiver_instance = HashMap::new();
        let mut by_receiver_determined_instance = HashMap::new();
        let mut by_unqualified_receiver_determined_instance = HashMap::new();
        for (key, name) in &instances {
            let normalized = key.with_unqualified_receiver();
            insert_unique_name(
                &mut by_unqualified_receiver_instance,
                normalized.clone(),
                name,
            );
            insert_unique_name(
                &mut by_receiver_determined_instance,
                key.without_type_arguments(),
                name,
            );
            insert_unique_name(
                &mut by_unqualified_receiver_determined_instance,
                normalized.without_type_arguments(),
                name,
            );
        }
        Self {
            by_definition: functions.into_iter().collect(),
            by_instance: instances.into_iter().collect(),
            by_unqualified_receiver_instance,
            by_receiver_determined_instance,
            by_unqualified_receiver_determined_instance,
            drops_by_definition_and_type: drops
                .into_iter()
                .map(|(definition, ty, name)| ((definition, drop_type_key(&ty)), name))
                .collect(),
            target_aliases: target_aliases.into_iter().collect(),
        }
    }

    fn name_for_definition(&self, definition: DefId) -> Option<&String> {
        self.by_definition.get(&definition)
    }

    pub(in crate::ir::lower) fn name_for_instance(
        &self,
        instance: &crate::mir::CallInstance,
        typed_hir: &TypedHir,
    ) -> Option<&String> {
        if let crate::mir::CallableIdentity::Definition(definition) = instance.callable
            && let Some(name) = self.name_for_definition(definition)
        {
            return Some(name);
        }
        let key = crate::mir::CallInstanceKey::from_instance(instance, typed_hir);
        key.as_ref()
            .and_then(|key| self.by_instance.get(key))
            .or_else(|| {
                key.as_ref()
                    .map(crate::mir::CallInstanceKey::with_unqualified_receiver)
                    .and_then(|key| self.by_unqualified_receiver_instance.get(&key))
                    .and_then(Option::as_ref)
            })
            .or_else(|| {
                key.as_ref()
                    .map(crate::mir::CallInstanceKey::without_type_arguments)
                    .and_then(|key| self.by_receiver_determined_instance.get(&key))
                    .and_then(Option::as_ref)
            })
            .or_else(|| {
                key.as_ref()
                    .map(crate::mir::CallInstanceKey::with_unqualified_receiver)
                    .map(|key| key.without_type_arguments())
                    .and_then(|key| self.by_unqualified_receiver_determined_instance.get(&key))
                    .and_then(Option::as_ref)
            })
            .or_else(|| match instance.callable {
                crate::mir::CallableIdentity::Definition(definition) => {
                    self.name_for_definition(definition)
                }
                _ => None,
            })
    }

    pub(in crate::ir::lower) fn name_for_drop(
        &self,
        definition: DefId,
        ty: &TypeExpr,
    ) -> Option<&String> {
        self.drops_by_definition_and_type
            .get(&(definition, drop_type_key(ty)))
            .or_else(|| self.name_for_definition(definition))
    }

    pub(in crate::ir::lower) fn target_alias(&self, name: &str) -> Option<&CallTarget> {
        self.target_aliases.get(name)
    }
}

fn insert_unique_name<K: std::hash::Hash + Eq>(
    names: &mut HashMap<K, Option<String>>,
    key: K,
    name: &str,
) {
    names
        .entry(key)
        .and_modify(|existing| {
            if existing.as_deref() != Some(name) {
                *existing = None;
            }
        })
        .or_insert_with(|| Some(name.to_string()));
}

fn drop_type_key(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Reference(reference) => short_runtime_type_name(&reference.name).to_string(),
        TypeExpr::Generic(generic) => format!(
            "{}<{}>",
            short_runtime_type_name(&generic.name),
            generic
                .arguments
                .iter()
                .map(crate::ast::canonical_type_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => crate::ast::canonical_type_expr(ty),
    }
}

fn short_runtime_type_name(name: &str) -> &str {
    name.rsplit(['.', '/']).next().unwrap_or(name)
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
