use crate::abi::{
    AbiType, ReturnPassing, ValueLayout, abi_value_from_type_expr_with_resolver,
    array_element_stride, layout_of, layout_struct,
};
use crate::ast::{
    CallExpr, Expr, IdentifierExpr, MemberExpr, TypeExpr, substitute_type_expr_parameters,
    type_expr_display_lossy,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, CallTarget, I32Location, SliceLocation, StrLocation, Type,
    U8Location, UsizeLocation,
};
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::{
    TypecheckFacts, TypecheckPayloadBindingMode, TypecheckScalarViewKind, TypecheckSliceElementKind,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::errors::ErrorPayload;
use super::types::type_expr_with_self_type;

pub(super) type ErrorPayloads = HashMap<CallTarget, ErrorPayload>;
pub(super) type ResolvedSources<'a> = HashMap<SourceId, &'a ResolveOutput>;

pub(super) struct LoweringContext<'a> {
    function_name: String,
    return_type: Type,
    function_return_type: Type,
    function_return_type_expr: Option<TypeExpr>,
    function_returns_optional: bool,
    function_signatures: FunctionSignatures,
    call_resolution: Option<CallResolution<'a>>,
    function_names: FunctionNames,
    generic_substitutions: HashMap<String, TypeExpr>,
    i32_parameters: Vec<Option<String>>,
    u8_parameters: Vec<Option<String>>,
    usize_parameters: Vec<Option<String>>,
    bool_parameters: Vec<Option<String>>,
    str_parameters: Vec<Option<String>>,
    slice_parameters: Vec<Option<SliceBinding>>,
    error_parameters: Vec<Option<String>>,
    reserved_local_abi_words: usize,
    locals: Vec<LocalBinding>,
    aggregate_fields: HashMap<usize, Vec<AggregateField>>,
    temporary_aggregate_drops: Vec<PendingAggregateDrop>,
    borrow_parameters: Vec<BorrowParameter>,
    aggregate_borrows: Vec<AggregateBorrowParameter>,
    error_payloads: ErrorPayloads,
    next_aggregate_slot_index: Rc<Cell<usize>>,
}

impl<'a> Clone for LoweringContext<'a> {
    fn clone(&self) -> Self {
        Self {
            function_name: self.function_name.clone(),
            return_type: self.return_type.clone(),
            function_return_type: self.function_return_type.clone(),
            function_return_type_expr: self.function_return_type_expr.clone(),
            function_returns_optional: self.function_returns_optional,
            function_signatures: self.function_signatures.clone(),
            call_resolution: self.call_resolution.clone(),
            function_names: self.function_names.clone(),
            generic_substitutions: self.generic_substitutions.clone(),
            i32_parameters: self.i32_parameters.clone(),
            u8_parameters: self.u8_parameters.clone(),
            usize_parameters: self.usize_parameters.clone(),
            bool_parameters: self.bool_parameters.clone(),
            str_parameters: self.str_parameters.clone(),
            slice_parameters: self.slice_parameters.clone(),
            error_parameters: self.error_parameters.clone(),
            reserved_local_abi_words: self.reserved_local_abi_words,
            locals: self.locals.clone(),
            aggregate_fields: self.aggregate_fields.clone(),
            temporary_aggregate_drops: self.temporary_aggregate_drops.clone(),
            borrow_parameters: self.borrow_parameters.clone(),
            aggregate_borrows: self.aggregate_borrows.clone(),
            error_payloads: self.error_payloads.clone(),
            next_aggregate_slot_index: self.next_aggregate_slot_index.clone(),
        }
    }
}

#[derive(Default)]
pub(super) struct LoweringParameterSlots {
    pub(super) i32: Vec<Option<String>>,
    pub(super) u8: Vec<Option<String>>,
    pub(super) usize: Vec<Option<String>>,
    pub(super) bool: Vec<Option<String>>,
    pub(super) str: Vec<Option<String>>,
    pub(super) slice: Vec<Option<SliceBinding>>,
    pub(super) error: Vec<Option<String>>,
    pub(super) borrow_parameters: Vec<BorrowParameter>,
    pub(super) aggregates: Vec<LoweringAggregateParameter>,
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
        let info = SliceTypeInfo {
            element_kind,
            element_type,
        };
        self.push_abi_word(
            None,
            None,
            None,
            None,
            None,
            Some(SliceBinding { name, info }),
            None,
        );
    }

    pub(super) fn push_error_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, None, None, None, Some(name));
        self.push_empty_abi_word();
        self.push_empty_abi_word();
        self.push_empty_abi_word();
    }

    pub(super) fn push_empty_abi_word(&mut self) {
        self.push_abi_word(None, None, None, None, None, None, None);
    }

    pub(super) fn reserve_empty_abi_words(&mut self, words: usize) -> usize {
        let start_index = self.next_parameter_index();
        for _ in 0..words {
            self.push_empty_abi_word();
        }
        start_index
    }

    pub(super) fn parameter_abi_word_count(&self) -> usize {
        debug_assert_eq!(self.i32.len(), self.u8.len());
        debug_assert_eq!(self.i32.len(), self.usize.len());
        debug_assert_eq!(self.i32.len(), self.bool.len());
        debug_assert_eq!(self.i32.len(), self.str.len());
        debug_assert_eq!(self.i32.len(), self.slice.len());
        debug_assert_eq!(self.i32.len(), self.error.len());
        self.i32.len()
    }

    fn next_parameter_index(&self) -> usize {
        self.i32.len()
    }

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
        self.bool.push(bool_name);
        self.str.push(str_name);
        self.slice.push(slice_name);
        self.error.push(error_name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoweringAggregateParameter {
    pub(super) name: String,
    pub(super) layout: ValueLayout,
    pub(super) slot_index: usize,
    pub(super) source: AggregateParameterSource,
    pub(super) is_copy: bool,
    pub(super) drop_kind: Option<AggregateDrop>,
    pub(super) fields: Vec<AggregateField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AggregateParameterSource {
    Indirect { parameter_index: usize },
    Direct { start_index: usize, words: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateBorrowParameter {
    pub(super) name: String,
    pub(super) layout: ValueLayout,
    pub(super) parameter_index: usize,
    pub(super) is_readwrite: bool,
    pub(super) fields: Vec<AggregateField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BorrowParameter {
    pub(super) name: String,
    pub(super) inner: Type,
    pub(super) parameter_index: usize,
    pub(super) is_readwrite: bool,
}

mod call_resolution;
mod construction;
mod drop_glue;
mod drop_obligation;
mod drop_queries;
mod enum_variants;
mod locals;

pub(super) use drop_glue::{
    aggregate_drop_for_type_expr_with_resolver, aggregate_drop_for_type_expr_with_resolver_ref,
    drop_glue_for_type_expr_with_resolver,
};
pub(super) use drop_obligation::{DropObligation, PayloadFieldDropState, StructFieldDropState};

fn call_target_for_source(source: SourceId, root_source: SourceId, name: String) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

fn std_os_imported_primitive_name(name: &str) -> bool {
    matches!(
        name,
        "syscall0"
            | "syscall1"
            | "syscall2"
            | "syscall3"
            | "syscall4"
            | "syscall5"
            | "syscall6"
            | "trap"
    )
}

#[derive(Clone)]
struct CallResolution<'a> {
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    resolved_sources: ResolvedSources<'a>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionNames {
    by_declaration_span: HashMap<ByteSpan, String>,
}

impl FunctionNames {
    pub(super) fn from_declarations(functions: Vec<(ByteSpan, String)>) -> Self {
        Self {
            by_declaration_span: functions.into_iter().collect(),
        }
    }

    fn name_for_declaration(&self, span: ByteSpan) -> Option<&String> {
        self.by_declaration_span.get(&span)
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionSignatures {
    signatures: HashMap<CallTarget, FunctionSignature>,
}

impl FunctionSignatures {
    #[cfg(test)]
    pub(super) fn new(return_types: HashMap<String, Type>) -> Self {
        Self {
            signatures: return_types
                .into_iter()
                .map(|(name, return_type)| {
                    (
                        CallTarget::same_file(name),
                        FunctionSignature {
                            return_type,
                            parameter_types: None,
                            parameter_abi_word_count: None,
                            success_return_passing: None,
                        },
                    )
                })
                .collect(),
        }
    }

    pub(super) fn from_call_targets(signatures: HashMap<CallTarget, FunctionSignature>) -> Self {
        Self { signatures }
    }

    pub(super) fn return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.signatures
            .get(target)
            .map(|signature| &signature.return_type)
    }

    pub(super) fn parameter_types(&self, target: &CallTarget) -> Option<&[Type]> {
        self.signatures
            .get(target)
            .and_then(|signature| signature.parameter_types.as_deref())
    }

    pub(super) fn parameter_abi_word_count(&self, target: &CallTarget) -> Option<usize> {
        self.signatures
            .get(target)
            .and_then(|signature| signature.parameter_abi_word_count)
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

#[derive(Clone)]
struct LocalBinding {
    name: String,
    kind: LocalKind,
    index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SliceBinding {
    pub(super) name: String,
    pub(super) info: SliceTypeInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SliceTypeInfo {
    pub(super) element_kind: TypecheckSliceElementKind,
    pub(super) element_type: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateLocal {
    pub(super) slot_index: usize,
    pub(super) layout: ValueLayout,
    pub(super) is_copy: bool,
    pub(super) drop_kind: Option<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingAggregateDrop {
    pub(super) name: String,
    pub(super) slot_index: usize,
    pub(super) layout: ValueLayout,
    pub(super) drop_kind: AggregateDrop,
    pub(super) obligation: DropObligation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AggregateDrop {
    Direct(DropGlue),
    Struct(StructDrop),
    Array(ArrayDrop),
    PayloadEnum(PayloadEnumDrop),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArrayDrop {
    pub(super) length: u64,
    pub(super) stride: u64,
    pub(super) element_layout: ValueLayout,
    pub(super) element_drop_kind: Box<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructDrop {
    pub(super) direct: Option<DropGlue>,
    pub(super) fields: Vec<StructDropField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructDropField {
    pub(super) offset: u32,
    pub(super) layout: ValueLayout,
    pub(super) drop_kind: Box<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DropGlue {
    pub(super) target: CallTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadEnumDrop {
    pub(super) variants: Vec<PayloadEnumDropVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadEnumDropVariant {
    pub(super) tag: u8,
    pub(super) fields: Vec<PayloadEnumDropField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadEnumDropField {
    pub(super) payload_offset: u32,
    pub(super) payload_layout: ValueLayout,
    pub(super) drop_kind: Box<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateField {
    pub(super) name: String,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_copy: bool,
    pub(super) drop_kind: Option<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateFieldAccess {
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_readwrite: bool,
    pub(super) is_copy: bool,
    pub(super) drop_kind: Option<AggregateDrop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AggregateFieldKind {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U16,
    U32,
    U64,
    U8,
    Usize,
    Bool,
    Str,
    Slice(SliceTypeInfo),
    Array {
        layout: ValueLayout,
        element: crate::abi::AbiType,
        length: u64,
        stride: u32,
    },
    Aggregate {
        layout: ValueLayout,
        fields: Vec<AggregateField>,
    },
}

impl AggregateFieldKind {
    pub(super) fn copy_aggregate_layout(&self) -> Option<ValueLayout> {
        match self {
            AggregateFieldKind::Array { layout, .. }
            | AggregateFieldKind::Aggregate { layout, .. } => Some(*layout),
            _ => None,
        }
    }

    pub(super) fn copy_aggregate_layout_and_fields(
        &self,
    ) -> Option<(ValueLayout, Vec<AggregateField>)> {
        match self {
            AggregateFieldKind::Array { layout, .. } => Some((*layout, Vec::new())),
            AggregateFieldKind::Aggregate { layout, fields } => Some((*layout, fields.clone())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice(SliceTypeInfo),
    Error,
    Aggregate {
        layout: ValueLayout,
        slot_index: usize,
        is_copy: bool,
        drop_obligation: DropObligation,
        drop_kind: Option<AggregateDrop>,
    },
}

impl LocalKind {
    fn abi_word_count(&self) -> usize {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => 1,
            Self::Str | Self::Slice(_) => 2,
            Self::Error => 4,
            Self::Aggregate { .. } => 0,
        }
    }
}
