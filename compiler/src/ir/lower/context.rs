use crate::abi::{ReturnPassing, ValueLayout};
use crate::ast::{CallExpr, Expr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, CallTarget, I32Location, SliceLocation, StrLocation, Type,
    U8Location, UsizeLocation,
};
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceId};
use crate::typecheck::TypecheckFacts;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use super::errors::ErrorPayload;

pub(super) type ErrorPayloads = HashMap<CallTarget, ErrorPayload>;

pub(super) struct LoweringContext<'a> {
    function_name: String,
    return_type: Type,
    function_return_type: Type,
    function_returns_optional: bool,
    function_signatures: FunctionSignatures,
    call_resolution: Option<CallResolution<'a>>,
    function_names: FunctionNames,
    i32_parameters: Vec<Option<String>>,
    u8_parameters: Vec<Option<String>>,
    usize_parameters: Vec<Option<String>>,
    bool_parameters: Vec<Option<String>>,
    str_parameters: Vec<Option<String>>,
    slice_parameters: Vec<Option<String>>,
    reserved_local_abi_words: usize,
    locals: Vec<LocalBinding>,
    aggregate_fields: HashMap<usize, Vec<AggregateField>>,
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
            function_returns_optional: self.function_returns_optional,
            function_signatures: self.function_signatures.clone(),
            call_resolution: self.call_resolution.clone(),
            function_names: self.function_names.clone(),
            i32_parameters: self.i32_parameters.clone(),
            u8_parameters: self.u8_parameters.clone(),
            usize_parameters: self.usize_parameters.clone(),
            bool_parameters: self.bool_parameters.clone(),
            str_parameters: self.str_parameters.clone(),
            slice_parameters: self.slice_parameters.clone(),
            reserved_local_abi_words: self.reserved_local_abi_words,
            locals: self.locals.clone(),
            aggregate_fields: self.aggregate_fields.clone(),
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
    pub(super) slice: Vec<Option<String>>,
    pub(super) aggregates: Vec<LoweringAggregateParameter>,
    pub(super) aggregate_borrows: Vec<AggregateBorrowParameter>,
}

impl LoweringParameterSlots {
    pub(super) fn push_i32_parameter(&mut self, name: String) {
        self.push_abi_word(Some(name), None, None, None, None, None);
    }

    pub(super) fn push_u8_parameter(&mut self, name: String) {
        self.push_abi_word(None, Some(name), None, None, None, None);
    }

    pub(super) fn push_usize_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, Some(name), None, None, None);
    }

    pub(super) fn push_bool_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, Some(name), None, None);
    }

    pub(super) fn push_str_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, None, Some(name), None);
    }

    pub(super) fn push_slice_parameter(&mut self, name: String) {
        self.push_abi_word(None, None, None, None, None, Some(name));
    }

    pub(super) fn push_empty_abi_word(&mut self) {
        self.push_abi_word(None, None, None, None, None, None);
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
        slice_name: Option<String>,
    ) {
        self.i32.push(i32_name);
        self.u8.push(u8_name);
        self.usize.push(usize_name);
        self.bool.push(bool_name);
        self.str.push(str_name);
        self.slice.push(slice_name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoweringAggregateParameter {
    pub(super) name: String,
    pub(super) layout: ValueLayout,
    pub(super) slot_index: usize,
    pub(super) source: AggregateParameterSource,
    pub(super) is_copy: bool,
    pub(super) drop_glue: Option<DropGlue>,
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

impl<'a> LoweringContext<'a> {
    pub(super) fn empty(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
    ) -> Self {
        Self {
            function_name,
            function_return_type: return_type.clone(),
            return_type,
            function_returns_optional: false,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            i32_parameters: Vec::new(),
            u8_parameters: Vec::new(),
            usize_parameters: Vec::new(),
            bool_parameters: Vec::new(),
            str_parameters: Vec::new(),
            slice_parameters: Vec::new(),
            reserved_local_abi_words: 0,
            locals: Vec::new(),
            aggregate_fields: HashMap::new(),
            aggregate_borrows: Vec::new(),
            error_payloads: ErrorPayloads::default(),
            next_aggregate_slot_index: Rc::new(Cell::new(0)),
        }
    }

    pub(super) fn new(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
        parameters: LoweringParameterSlots,
    ) -> Self {
        let mut locals = Vec::new();
        let mut aggregate_fields = HashMap::new();
        let next_aggregate_slot_index = parameters
            .aggregates
            .iter()
            .map(|parameter| parameter.slot_index + 1)
            .max()
            .unwrap_or(0);
        for parameter in parameters.aggregates {
            locals.push(LocalBinding {
                name: parameter.name,
                kind: LocalKind::Aggregate {
                    layout: parameter.layout,
                    slot_index: parameter.slot_index,
                    is_copy: parameter.is_copy,
                    drop_state: AggregateDropState::from_drop_glue(&parameter.drop_glue),
                    drop_glue: parameter.drop_glue,
                },
                index: 0,
            });
            aggregate_fields.insert(parameter.slot_index, parameter.fields);
        }

        Self {
            function_name,
            function_return_type: return_type.clone(),
            return_type,
            function_returns_optional: false,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            i32_parameters: parameters.i32,
            u8_parameters: parameters.u8,
            usize_parameters: parameters.usize,
            bool_parameters: parameters.bool,
            str_parameters: parameters.str,
            slice_parameters: parameters.slice,
            reserved_local_abi_words: 0,
            locals,
            aggregate_fields,
            aggregate_borrows: parameters.aggregate_borrows,
            error_payloads: ErrorPayloads::default(),
            next_aggregate_slot_index: Rc::new(Cell::new(next_aggregate_slot_index)),
        }
    }

    pub(super) fn with_call_resolution(
        mut self,
        root_source: SourceId,
        resolved: &'a ResolveOutput,
        typecheck_facts: &'a TypecheckFacts,
        function_names: FunctionNames,
    ) -> Self {
        self.call_resolution = Some(CallResolution {
            root_source,
            resolved,
            typecheck_facts,
        });
        self.function_names = function_names;
        self
    }

    pub(super) fn with_function_return_type(mut self, return_type: Type) -> Self {
        self.function_return_type = return_type;
        self
    }

    pub(super) fn with_function_returns_optional(
        mut self,
        function_returns_optional: bool,
    ) -> Self {
        self.function_returns_optional = function_returns_optional;
        self
    }

    pub(super) fn with_error_payloads(mut self, error_payloads: ErrorPayloads) -> Self {
        self.error_payloads = error_payloads;
        self
    }

    pub(super) fn function_name(&self) -> &str {
        &self.function_name
    }

    pub(super) fn return_type(&self) -> &Type {
        &self.return_type
    }

    pub(super) fn function_return_type(&self) -> &Type {
        &self.function_return_type
    }

    pub(super) fn function_returns_optional(&self) -> bool {
        self.function_returns_optional
    }

    pub(super) fn call_return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.function_signatures.return_type(target)
    }

    pub(super) fn call_parameter_types(&self, target: &CallTarget) -> Option<&[Type]> {
        self.function_signatures.parameter_types(target)
    }

    pub(super) fn call_parameter_abi_word_count(&self, target: &CallTarget) -> Option<usize> {
        self.function_signatures.parameter_abi_word_count(target)
    }

    pub(super) fn call_success_return_passing(&self, target: &CallTarget) -> Option<ReturnPassing> {
        self.function_signatures.success_return_passing(target)
    }

    pub(super) fn direct_call_target_and_name(
        &self,
        call: &CallExpr,
    ) -> Option<(CallTarget, String)> {
        match call.callee.as_ref() {
            Expr::Identifier(identifier) => Some((
                self.call_target(call, &identifier.name),
                identifier.name.clone(),
            )),
            Expr::Member(_) => {
                let resolution = self.call_resolution.as_ref()?;
                if let Some((target, target_name)) = self.method_call_target_and_name(call) {
                    return Some((target, target_name));
                }
                let (_owner, function) = resolution.resolved.associated_function_for_call(call)?;
                let target = call_target_for_source(
                    function.name_span.source,
                    resolution.root_source,
                    function.target_name.clone(),
                );
                Some((target, function.target_name.clone()))
            }
            _ => None,
        }
    }

    pub(super) fn error_payload_for_call(&self, call: &CallExpr) -> Option<ErrorPayload> {
        let (target, _) = self.direct_call_target_and_name(call)?;
        self.error_payloads.get(&target).cloned()
    }

    pub(super) fn call_target(&self, call: &CallExpr, fallback_name: &str) -> CallTarget {
        let Some(resolution) = &self.call_resolution else {
            return CallTarget::same_file(fallback_name);
        };
        if let Some((_owner, function)) = resolution.resolved.associated_function_for_call(call) {
            return call_target_for_source(
                function.name_span.source,
                resolution.root_source,
                function.target_name.clone(),
            );
        }
        if let Some((target, _name)) = self.method_call_target_and_name(call) {
            return target;
        }

        let Some(symbol) = resolution.resolved.symbol_for_call(call) else {
            return CallTarget::same_file(fallback_name);
        };

        match &symbol.kind {
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_)
                if symbol.declaration_span.source != resolution.root_source =>
            {
                let target_name = self
                    .function_names
                    .name_for_declaration(symbol.declaration_span)
                    .unwrap_or(&symbol.name);
                CallTarget::imported(symbol.declaration_span.source, target_name.clone())
            }
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
                CallTarget::same_file(symbol.name.clone())
            }
            SymbolKind::Imported(_) => CallTarget::same_file(fallback_name),
        }
    }

    pub(super) fn primitive_name_for_call(&self, call: &CallExpr) -> Option<&str> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution.resolved.symbol_for_call(call)?;
        match &symbol.kind {
            SymbolKind::Primitive(_) => Some(symbol.name.as_str()),
            SymbolKind::Function(_) | SymbolKind::Type(_) | SymbolKind::Imported(_) => None,
        }
    }

    pub(super) fn resolved_calls(&self) -> Option<(SourceId, &'a ResolveOutput)> {
        self.call_resolution
            .as_ref()
            .map(|resolution| (resolution.root_source, resolution.resolved))
    }

    pub(super) fn method_call_receiver<'b>(&self, call: &'b CallExpr) -> Option<&'b Expr> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        Some(&member.object)
    }

    fn method_call_target_and_name(&self, call: &CallExpr) -> Option<(CallTarget, String)> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let method_name_span = resolution
            .typecheck_facts
            .method_call_target(member.member_span)?;
        let target_name = self
            .function_names
            .name_for_declaration(method_name_span)?
            .clone();
        let target = call_target_for_source(
            method_name_span.source,
            resolution.root_source,
            target_name.clone(),
        );
        Some((target, target_name))
    }

    pub(super) fn next_i32_local_location(&self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(I32Location::Local)
    }

    pub(super) fn next_u8_local_location(&self) -> Result<U8Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(U8Location::Local)
    }

    pub(super) fn next_usize_local_location(&self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(UsizeLocation::Local)
    }

    pub(super) fn first_temporary_local_index(&self) -> Result<usize, Vec<Diagnostic>> {
        Ok(self.used_local_abi_words())
    }

    pub(super) fn next_bool_local_location(&self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(BoolLocation::Local)
    }

    pub(super) fn next_str_local_location(&self) -> Result<StrLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(StrLocation::Local)
    }

    pub(super) fn next_slice_local_location(&self) -> Result<SliceLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(SliceLocation::Local)
    }

    pub(super) fn with_reserved_local_abi_words(&self, words: usize) -> Self {
        let mut context = self.clone();
        context.reserved_local_abi_words += words;
        context
    }

    pub(super) fn define_i32_local(&mut self, name: String) {
        self.define_local(name, LocalKind::I32);
    }

    pub(super) fn define_u8_local(&mut self, name: String) {
        self.define_local(name, LocalKind::U8);
    }

    pub(super) fn define_usize_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Usize);
    }

    pub(super) fn define_bool_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Bool);
    }

    pub(super) fn define_str_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Str);
    }

    pub(super) fn define_slice_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Slice);
    }

    pub(super) fn define_aggregate_local(
        &mut self,
        name: String,
        layout: ValueLayout,
        is_copy: bool,
        drop_glue: Option<DropGlue>,
        fields: Vec<AggregateField>,
    ) -> usize {
        let slot_index = self.reserve_aggregate_slot_index();
        self.locals.push(LocalBinding {
            name,
            kind: LocalKind::Aggregate {
                layout,
                slot_index,
                is_copy,
                drop_state: AggregateDropState::from_drop_glue(&drop_glue),
                drop_glue,
            },
            index: 0,
        });
        self.aggregate_fields.insert(slot_index, fields);
        slot_index
    }

    pub(super) fn define_error_local(
        &mut self,
        name: String,
    ) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
        let index = self.next_local_index(LocalKind::Error.abi_word_count())?;
        self.locals.push(LocalBinding {
            name,
            kind: LocalKind::Error,
            index,
        });
        Ok((StrLocation::Local(index), StrLocation::Local(index + 2)))
    }

    pub(super) fn next_error_local_locations(
        &self,
    ) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
        let index = self.next_local_index(LocalKind::Error.abi_word_count())?;
        Ok((StrLocation::Local(index), StrLocation::Local(index + 2)))
    }

    pub(super) fn i32_location(&self, name: &str) -> Option<I32Location> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::I32)
            .map(|local| I32Location::Local(local.index))
            .or_else(|| {
                self.i32_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(I32Location::Parameter)
            })
    }

    pub(super) fn usize_location(&self, name: &str) -> Option<UsizeLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Usize)
            .map(|local| UsizeLocation::Local(local.index))
            .or_else(|| {
                self.usize_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(UsizeLocation::Parameter)
            })
    }

    pub(super) fn u8_location(&self, name: &str) -> Option<U8Location> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::U8)
            .map(|local| U8Location::Local(local.index))
            .or_else(|| {
                self.u8_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(U8Location::Parameter)
            })
    }

    pub(super) fn bool_location(&self, name: &str) -> Option<BoolLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Bool)
            .map(|local| BoolLocation::Local(local.index))
            .or_else(|| {
                self.bool_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(BoolLocation::Parameter)
            })
    }

    pub(super) fn str_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Str)
            .map(|local| StrLocation::Local(local.index))
            .or_else(|| {
                self.str_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(StrLocation::Parameter)
            })
    }

    pub(super) fn slice_location(&self, name: &str) -> Option<SliceLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Slice)
            .map(|local| SliceLocation::Local(local.index))
            .or_else(|| {
                self.slice_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(SliceLocation::Parameter)
            })
    }

    pub(super) fn error_code_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Error)
            .map(|local| StrLocation::Local(local.index))
    }

    pub(super) fn error_message_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Error)
            .map(|local| StrLocation::Local(local.index + 2))
    }

    pub(super) fn aggregate_slot(&self, name: &str) -> Option<(usize, ValueLayout)> {
        self.aggregate_local(name)
            .map(|local| (local.slot_index, local.layout))
    }

    pub(super) fn aggregate_local(&self, name: &str) -> Option<AggregateLocal> {
        self.locals.iter().find_map(|local| {
            if local.name == name
                && let LocalKind::Aggregate {
                    layout,
                    slot_index,
                    is_copy,
                    ref drop_glue,
                    ..
                } = local.kind
            {
                return Some(AggregateLocal {
                    slot_index,
                    layout,
                    is_copy,
                    drop_glue: drop_glue.clone(),
                });
            }
            None
        })
    }

    pub(super) fn aggregate_local_by_slot(&self, slot_index: usize) -> Option<AggregateLocal> {
        self.locals.iter().find_map(|local| {
            let LocalKind::Aggregate {
                layout,
                slot_index: local_slot_index,
                is_copy,
                ref drop_glue,
                ..
            } = local.kind
            else {
                return None;
            };
            if local_slot_index == slot_index {
                return Some(AggregateLocal {
                    slot_index: local_slot_index,
                    layout,
                    is_copy,
                    drop_glue: drop_glue.clone(),
                });
            }
            None
        })
    }

    pub(super) fn aggregate_local_fields(&self, name: &str) -> Option<Vec<AggregateField>> {
        let local = self.aggregate_local(name)?;
        self.aggregate_fields.get(&local.slot_index).cloned()
    }

    pub(super) fn mark_aggregate_local_dropped(&mut self, name: &str) {
        self.update_aggregate_drop_state(name, AggregateDropState::Suppressed);
    }

    pub(super) fn mark_aggregate_local_moved(&mut self, name: &str) {
        self.update_aggregate_drop_state(name, AggregateDropState::Suppressed);
    }

    pub(super) fn mark_aggregate_local_initialized(&mut self, name: &str) {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return;
        };
        let LocalKind::Aggregate {
            drop_glue,
            drop_state,
            ..
        } = &mut local.kind
        else {
            return;
        };
        *drop_state = AggregateDropState::from_drop_glue(drop_glue);
    }

    pub(super) fn pending_aggregate_drops(&self) -> Vec<PendingAggregateDrop> {
        self.locals
            .iter()
            .rev()
            .filter_map(|local| {
                let LocalKind::Aggregate {
                    layout,
                    slot_index,
                    drop_state,
                    ref drop_glue,
                    ..
                } = local.kind
                else {
                    return None;
                };
                if drop_state != AggregateDropState::NeedsDrop {
                    return None;
                }
                Some(PendingAggregateDrop {
                    name: local.name.clone(),
                    slot_index,
                    layout,
                    drop_glue: drop_glue.clone()?,
                })
            })
            .collect()
    }

    pub(super) fn local_mark(&self) -> usize {
        self.locals.len()
    }

    pub(super) fn aggregate_local_defined_since(&self, name: &str, local_mark: usize) -> bool {
        self.locals
            .get(local_mark..)
            .unwrap_or(&[])
            .iter()
            .any(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
    }

    pub(super) fn local_defined_since(&self, name: &str, local_mark: usize) -> bool {
        self.locals
            .get(local_mark..)
            .unwrap_or(&[])
            .iter()
            .any(|local| local.name == name)
    }

    pub(super) fn pending_aggregate_drops_since(
        &self,
        local_mark: usize,
    ) -> Vec<PendingAggregateDrop> {
        let locals = self.locals.get(local_mark..).unwrap_or(&[]);
        locals
            .iter()
            .rev()
            .filter_map(|local| {
                let LocalKind::Aggregate {
                    layout,
                    slot_index,
                    drop_state,
                    ref drop_glue,
                    ..
                } = local.kind
                else {
                    return None;
                };
                if drop_state != AggregateDropState::NeedsDrop {
                    return None;
                }
                Some(PendingAggregateDrop {
                    name: local.name.clone(),
                    slot_index,
                    layout,
                    drop_glue: drop_glue.clone()?,
                })
            })
            .collect()
    }

    pub(super) fn pending_aggregate_drop_by_slot(
        &self,
        slot_index: usize,
    ) -> Option<PendingAggregateDrop> {
        self.locals.iter().find_map(|local| {
            let LocalKind::Aggregate {
                layout,
                slot_index: local_slot_index,
                drop_state,
                ref drop_glue,
                ..
            } = local.kind
            else {
                return None;
            };
            if local_slot_index != slot_index || drop_state != AggregateDropState::NeedsDrop {
                return None;
            }
            Some(PendingAggregateDrop {
                name: local.name.clone(),
                slot_index,
                layout,
                drop_glue: drop_glue.clone()?,
            })
        })
    }

    pub(super) fn aggregate_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        self.aggregate_local_field(aggregate_name, field_name)
            .or_else(|| self.aggregate_borrow_field(aggregate_name, field_name))
    }

    fn aggregate_local_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        let aggregate = self.aggregate_local(aggregate_name)?;
        self.aggregate_fields
            .get(&aggregate.slot_index)?
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| AggregateFieldAccess {
                source: AggregateLocation::Slot(aggregate.slot_index),
                offset: field.offset,
                kind: field.kind.clone(),
                is_readwrite: true,
                is_copy: field.is_copy,
                drop_glue: field.drop_glue.clone(),
            })
    }

    fn aggregate_borrow_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        let borrow = self
            .aggregate_borrows
            .iter()
            .find(|borrow| borrow.name == aggregate_name)?;
        borrow
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| AggregateFieldAccess {
                source: AggregateLocation::Parameter(borrow.parameter_index),
                offset: field.offset,
                kind: field.kind.clone(),
                is_readwrite: borrow.is_readwrite,
                is_copy: field.is_copy,
                drop_glue: field.drop_glue.clone(),
            })
    }

    pub(super) fn aggregate_borrow_parameter(
        &self,
        aggregate_name: &str,
    ) -> Option<&AggregateBorrowParameter> {
        self.aggregate_borrows
            .iter()
            .find(|borrow| borrow.name == aggregate_name)
    }

    fn next_local_index(&self, required_words: usize) -> Result<usize, Vec<Diagnostic>> {
        let index = self.used_local_abi_words();
        if index + required_words > MAX_LOCAL_ABI_WORDS {
            return Err(vec![Diagnostic::error(
                "E8008",
                format!("IR v0 can only lower up to {MAX_LOCAL_ABI_WORDS} local ABI words"),
            )]);
        }

        Ok(index)
    }

    fn define_local(&mut self, name: String, kind: LocalKind) {
        let index = self.used_local_abi_words();
        self.locals.push(LocalBinding { name, kind, index });
    }

    fn update_aggregate_drop_state(&mut self, name: &str, state: AggregateDropState) {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return;
        };
        let LocalKind::Aggregate { drop_state, .. } = &mut local.kind else {
            return;
        };
        *drop_state = state;
    }

    fn used_local_abi_words(&self) -> usize {
        self.reserved_local_abi_words
            + self
                .locals
                .iter()
                .map(|local| local.kind.abi_word_count())
                .sum::<usize>()
    }

    pub(super) fn reserve_aggregate_slot_index(&self) -> usize {
        let slot_index = self.next_aggregate_slot_index.get();
        self.next_aggregate_slot_index.set(slot_index + 1);
        slot_index
    }

    pub(super) fn aggregate_slot_mark(&self) -> usize {
        self.next_aggregate_slot_index.get()
    }

    pub(super) fn restore_aggregate_slot_mark(&self, mark: usize) {
        self.next_aggregate_slot_index.set(mark);
    }

    pub(super) fn aggregate_slot_counter(&self) -> Rc<Cell<usize>> {
        self.next_aggregate_slot_index.clone()
    }

    pub(super) fn drop_glue_for_type_expr(&self, ty: &TypeExpr) -> Option<DropGlue> {
        let (root_source, resolved) = self.resolved_calls()?;
        drop_glue_for_type_expr(ty, root_source, resolved)
    }
}

fn call_target_for_source(source: SourceId, root_source: SourceId, name: String) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

#[derive(Clone)]
struct CallResolution<'a> {
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
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
pub(super) struct AggregateLocal {
    pub(super) slot_index: usize,
    pub(super) layout: ValueLayout,
    pub(super) is_copy: bool,
    pub(super) drop_glue: Option<DropGlue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingAggregateDrop {
    pub(super) name: String,
    pub(super) slot_index: usize,
    pub(super) layout: ValueLayout,
    pub(super) drop_glue: DropGlue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DropGlue {
    pub(super) target: CallTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateField {
    pub(super) name: String,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_copy: bool,
    pub(super) drop_glue: Option<DropGlue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AggregateFieldAccess {
    pub(super) source: AggregateLocation,
    pub(super) offset: u32,
    pub(super) kind: AggregateFieldKind,
    pub(super) is_readwrite: bool,
    pub(super) is_copy: bool,
    pub(super) drop_glue: Option<DropGlue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AggregateFieldKind {
    I32,
    U16,
    U32,
    U8,
    Usize,
    Bool,
    Aggregate {
        layout: ValueLayout,
        fields: Vec<AggregateField>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice,
    Error,
    Aggregate {
        layout: ValueLayout,
        slot_index: usize,
        is_copy: bool,
        drop_state: AggregateDropState,
        drop_glue: Option<DropGlue>,
    },
}

impl LocalKind {
    fn abi_word_count(&self) -> usize {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Bool => 1,
            Self::Str | Self::Slice => 2,
            Self::Error => 4,
            Self::Aggregate { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateDropState {
    NeedsDrop,
    Suppressed,
}

impl AggregateDropState {
    fn from_drop_glue(drop_glue: &Option<DropGlue>) -> Self {
        if drop_glue.is_some() {
            Self::NeedsDrop
        } else {
            Self::Suppressed
        }
    }
}

pub(super) fn drop_glue_for_type_expr(
    ty: &TypeExpr,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Option<DropGlue> {
    match ty {
        TypeExpr::Fallible(fallible) => {
            return drop_glue_for_type_expr(&fallible.success, root_source, resolved);
        }
        TypeExpr::Optional(optional) => {
            return drop_glue_for_type_expr(&optional.inner, root_source, resolved);
        }
        _ => {}
    }

    let reference = match ty {
        TypeExpr::Reference(reference) => reference,
        _ => return None,
    };
    let (symbol, type_symbol) =
        resolved.type_symbol_definition_by_reference_name(&reference.name)?;
    let drop_member = type_symbol.drop_member.as_ref()?;
    let target = if symbol.declaration_span.source == root_source {
        CallTarget::same_file(drop_member.target_name.clone())
    } else {
        CallTarget::imported(
            symbol.declaration_span.source,
            drop_member.target_name.clone(),
        )
    };
    Some(DropGlue { target })
}

const MAX_LOCAL_ABI_WORDS: usize = 7;
