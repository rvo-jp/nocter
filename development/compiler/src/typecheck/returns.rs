use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_call_signature, resolved_method_for_call};
use super::copyability::implicit_non_copy_owned_value_source;
use super::diagnostics::{
    body_result_type_mismatch_diagnostic, borrow_return_escapes_diagnostic,
    catch_block_fallthrough_diagnostic, fallible_success_error_diagnostic,
    missing_return_diagnostic, missing_return_value_diagnostic, never_return_statement_diagnostic,
    non_copy_struct_return_diagnostic, return_type_mismatch_diagnostic,
    unexpected_body_result_diagnostic, unexpected_return_value_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_method, environment_for_parameters_in_impl,
    environment_for_switch_arm, impl_member_name,
};
use super::expressions::expression_type;
use super::fallible::{check_catch_operand, check_propagation};
use super::model::{CallableKind, ReturnContext, Type, TypeEnvironment, binding_kind_is_mutable};
use super::numeric::integer_literal_expr_value;
use super::operations::is_expression_assignable;
use super::type_expr::{type_expr_to_type_in_environment, type_expr_to_type_with_substitutions};
use super::variants::{is_enum_variant_call, switch_statement_covers_all_variants};
use crate::ast::{
    AstFile, Block, Expr, IfIsStmt, ImplDecl, ImplMember, InterpolatedStringPart, Item, ReturnStmt,
    Stmt, SwitchArm, SwitchPayloadBinding, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{LocalSymbolKind, ResolveOutput, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Default)]
struct BorrowReturnEnvironment {
    bindings: HashMap<String, BorrowReturnProvenance>,
}

type BorrowReturnSummaries = HashMap<ByteSpan, BorrowReturnProvenance>;

impl BorrowReturnEnvironment {
    fn get(&self, name: &str) -> Option<&BorrowReturnProvenance> {
        self.bindings.get(name)
    }

    fn define_binding(
        &mut self,
        name: String,
        contains_borrow_like: bool,
        provenance: Option<BorrowReturnProvenance>,
    ) {
        if contains_borrow_like {
            if let Some(provenance) = provenance {
                self.bindings.insert(name, provenance);
            } else {
                self.bindings.remove(&name);
            }
        } else {
            self.bindings.remove(&name);
        }
    }

    fn join_reachable(&mut self, states: &[BorrowReturnEnvironment]) {
        let mut joined = HashMap::new();
        for state in states {
            for (name, provenance) in &state.bindings {
                joined
                    .entry(name.clone())
                    .and_modify(|existing: &mut BorrowReturnProvenance| existing.merge(provenance))
                    .or_insert_with(|| provenance.clone());
            }
        }
        self.bindings = joined;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BorrowReturnProvenance {
    Static,
    InputBorrow {
        sources: BTreeSet<String>,
    },
    Escaping {
        source: String,
    },
    Aggregate {
        fallback: Option<Box<BorrowReturnProvenance>>,
        fields: BTreeMap<String, BorrowReturnProvenance>,
        elements: BTreeMap<usize, BorrowReturnProvenance>,
    },
}

impl BorrowReturnProvenance {
    fn input_borrow(source: String) -> Self {
        Self::InputBorrow {
            sources: BTreeSet::from([source]),
        }
    }

    fn escaping(source: String) -> Self {
        Self::Escaping { source }
    }

    fn escaping_source(&self) -> Option<&str> {
        match self {
            Self::Escaping { source } => Some(source),
            Self::Aggregate {
                fallback,
                fields,
                elements,
            } => fallback
                .as_deref()
                .and_then(BorrowReturnProvenance::escaping_source)
                .or_else(|| {
                    fields
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                })
                .or_else(|| {
                    elements
                        .values()
                        .find_map(BorrowReturnProvenance::escaping_source)
                }),
            Self::Static | Self::InputBorrow { .. } => None,
        }
    }

    fn field_provenance(&self, field: &str) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, fields, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut provenance, fields.get(field).cloned());
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    fn element_provenance(&self, index: Option<usize>) -> Option<BorrowReturnProvenance> {
        match self {
            Self::Aggregate {
                fallback, elements, ..
            } => {
                let mut provenance = fallback.as_deref().cloned();
                if let Some(index) = index {
                    merge_borrow_return_provenance(&mut provenance, elements.get(&index).cloned());
                } else {
                    for element_provenance in elements.values() {
                        merge_borrow_return_provenance(
                            &mut provenance,
                            Some(element_provenance.clone()),
                        );
                    }
                }
                provenance
            }
            _ => Some(self.clone()),
        }
    }

    fn merge(&mut self, other: &BorrowReturnProvenance) {
        match (&mut *self, other) {
            (Self::Escaping { .. }, _) => {}
            (_, Self::Escaping { source }) => {
                *self = Self::Escaping {
                    source: source.clone(),
                };
            }
            (
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
                Self::Aggregate {
                    fallback: other_fallback,
                    fields: other_fields,
                    elements: other_elements,
                },
            ) => {
                merge_borrow_return_boxed_provenance(fallback, other_fallback.as_deref().cloned());
                for (field, other_field_provenance) in other_fields {
                    fields
                        .entry(field.clone())
                        .and_modify(|field_provenance| {
                            field_provenance.merge(other_field_provenance)
                        })
                        .or_insert_with(|| other_field_provenance.clone());
                }
                for (index, other_element_provenance) in other_elements {
                    elements
                        .entry(*index)
                        .and_modify(|element_provenance| {
                            element_provenance.merge(other_element_provenance)
                        })
                        .or_insert_with(|| other_element_provenance.clone());
                }
            }
            (
                Self::Aggregate {
                    fallback,
                    fields: _,
                    elements: _,
                },
                other,
            ) => {
                merge_borrow_return_boxed_provenance(fallback, Some(other.clone()));
            }
            (
                existing,
                Self::Aggregate {
                    fallback,
                    fields,
                    elements,
                },
            ) => {
                let mut merged_fallback = fallback.as_deref().cloned();
                merge_borrow_return_provenance(&mut merged_fallback, Some(existing.clone()));
                *existing = Self::Aggregate {
                    fallback: merged_fallback.map(Box::new),
                    fields: fields.clone(),
                    elements: elements.clone(),
                };
            }
            (
                Self::InputBorrow { sources },
                Self::InputBorrow {
                    sources: other_sources,
                },
            ) => {
                sources.extend(other_sources.iter().cloned());
            }
            (Self::Static, Self::InputBorrow { sources }) => {
                *self = Self::InputBorrow {
                    sources: sources.clone(),
                };
            }
            (Self::InputBorrow { .. }, Self::Static) | (Self::Static, Self::Static) => {}
        }
    }
}

pub(super) fn check_return_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let summaries = borrow_return_summaries(ast, resolved);
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let mut environment = environment_for_function(function, resolved);
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                let context = ReturnContext::new(
                    if function.owner.is_some() {
                        CallableKind::AssociatedFunction(function.name.clone())
                    } else {
                        CallableKind::Function(function.name.clone())
                    },
                    type_expr_to_type_in_environment(&function.return_type, resolved, &environment),
                    function.return_type.span(),
                );
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    &function.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    &summaries,
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_return_types(sources, impl_, resolved, diagnostics, &summaries);
            }
            _ => {}
        }
    }
}

fn borrow_return_summaries(ast: &AstFile, resolved: &ResolveOutput) -> BorrowReturnSummaries {
    let mut summaries = BorrowReturnSummaries::new();
    for _ in 0..=borrow_return_callable_count(ast) {
        let next = collect_borrow_return_summaries(ast, resolved, &summaries);
        if next == summaries {
            return summaries;
        }
        summaries = next;
    }
    summaries
}

fn borrow_return_callable_count(ast: &AstFile) -> usize {
    ast.items
        .iter()
        .map(|item| match item {
            Item::Function(_) => 1,
            Item::Impl(impl_) => impl_
                .members
                .iter()
                .filter(
                    |member| matches!(member, ImplMember::Method(method) if method.body.is_some()),
                )
                .count(),
            _ => 0,
        })
        .sum()
}

fn collect_borrow_return_summaries(
    ast: &AstFile,
    resolved: &ResolveOutput,
    previous: &BorrowReturnSummaries,
) -> BorrowReturnSummaries {
    let mut summaries = BorrowReturnSummaries::new();
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let environment = environment_for_function(function, resolved);
                let return_type =
                    type_expr_to_type_in_environment(&function.return_type, resolved, &environment);
                if let Some(provenance) = borrow_return_provenance_for_callable_body(
                    &function.body,
                    &return_type,
                    resolved,
                    &environment,
                    previous,
                ) {
                    summaries.insert(function_summary_key(function), provenance);
                }
            }
            Item::Impl(impl_) => {
                for member in &impl_.members {
                    let ImplMember::Method(method) = member else {
                        continue;
                    };
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let environment = environment_for_method(method, resolved, impl_);
                    let return_type = type_expr_to_type_in_environment(
                        &method.return_type,
                        resolved,
                        &environment,
                    );
                    if let Some(provenance) = borrow_return_provenance_for_callable_body(
                        body,
                        &return_type,
                        resolved,
                        &environment,
                        previous,
                    ) {
                        summaries.insert(method.name_span, provenance);
                    }
                }
            }
            _ => {}
        }
    }
    summaries
}

fn function_summary_key(function: &crate::ast::FunctionDecl) -> ByteSpan {
    if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    }
}

fn borrow_return_provenance_for_callable_body(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(return_type, resolved) {
        return None;
    }

    let mut provenance = None;
    let mut body_environment = environment.clone();
    let mut body_borrow_provenance = BorrowReturnEnvironment::default();
    collect_return_statement_provenance(
        block,
        resolved,
        &mut body_environment,
        &mut body_borrow_provenance,
        summaries,
        &mut provenance,
    );
    merge_borrow_return_provenance(
        &mut provenance,
        borrow_return_provenance_for_block_result(
            block,
            resolved,
            environment,
            &BorrowReturnEnvironment::default(),
            summaries,
        ),
    );
    provenance
}

fn collect_return_statement_provenance(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    provenance: &mut Option<BorrowReturnProvenance>,
) {
    for statement in &block.statements {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    let actual = expression_type(expression, resolved, environment);
                    merge_borrow_return_provenance(
                        provenance,
                        borrow_return_provenance_for_expression(
                            expression,
                            &actual,
                            resolved,
                            environment,
                            borrow_provenance,
                            summaries,
                        ),
                    );
                }
            }
            Stmt::If(if_statement) => {
                let mut then_environment = environment.clone();
                let mut then_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &if_statement.then_block,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    provenance,
                );
                if let Some(else_block) = &if_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        provenance,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::IfIs(if_is_statement) => {
                let mut then_environment =
                    environment_for_if_is_binding(if_is_statement, resolved, environment);
                let mut then_borrow_provenance = borrow_provenance.clone();
                define_if_is_payload_borrow_return_binding(
                    if_is_statement,
                    resolved,
                    environment,
                    &then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                );
                collect_return_statement_provenance(
                    &if_is_statement.then_block,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    provenance,
                );
                if let Some(else_block) = &if_is_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        provenance,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::Switch(switch_statement) => {
                for arm in &switch_statement.arms {
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                    );
                    let mut arm_borrow_provenance = borrow_provenance.clone();
                    define_switch_arm_payload_borrow_return_binding(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                        &arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                    );
                    collect_return_statement_provenance(
                        &arm.body,
                        resolved,
                        &mut arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                        provenance,
                    );
                }
                if let Some(wildcard_arm) = &switch_statement.wildcard_arm {
                    let mut wildcard_environment = environment.clone();
                    let mut wildcard_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        &wildcard_arm.body,
                        resolved,
                        &mut wildcard_environment,
                        &mut wildcard_borrow_provenance,
                        summaries,
                        provenance,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::While(statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    provenance,
                );
            }
            Stmt::ForRange(statement) => {
                let mut body_environment =
                    environment_for_for_range_binding(statement, resolved, environment);
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    provenance,
                );
            }
            Stmt::Loop(statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &statement.body,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    provenance,
                );
            }
            _ => {
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return;
        }
    }
}

fn check_impl_member_return_types(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &BorrowReturnSummaries,
) {
    for member in &impl_.members {
        match member {
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, impl_);
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                let context = ReturnContext::new(
                    CallableKind::Method(impl_member_name(impl_, &method.name)),
                    type_expr_to_type_in_environment(&method.return_type, resolved, &environment),
                    method.return_type.span(),
                );
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    summaries,
                );
            }
            ImplMember::Drop(drop_) => {
                let context = ReturnContext::new(
                    CallableKind::Drop(impl_member_name(impl_, "drop")),
                    Type::Void,
                    drop_.binding.ty.span(),
                );
                let mut environment = environment_for_parameters_in_impl(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    impl_,
                );
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                check_block_returns(
                    sources,
                    &drop_.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    summaries,
                );
            }
        }
    }
}

fn check_fallible_success_type(
    sources: &SourceMap,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Type::Fallible { success, .. } = &context.declared_type else {
        return;
    };

    if success_type_accepts_bare_error(success) {
        diagnostics.push(fallible_success_error_diagnostic(sources, context));
    }
}

fn success_type_accepts_bare_error(ty: &Type) -> bool {
    match ty {
        Type::Error => true,
        Type::Optional(inner) => success_type_accepts_bare_error(inner),
        _ => false,
    }
}

fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    if context.success_type().first_unsized_part().is_some() {
        return;
    }

    let block_exits = check_block_return_statements(
        sources,
        block,
        context,
        resolved,
        diagnostics,
        environment,
        borrow_provenance,
        summaries,
    );

    if block_exits {
        return;
    }

    if let Some(result) = &block.result {
        check_body_result_return(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return;
    }

    if context.requires_explicit_return()
        && !block_guarantees_return_or_never(block, resolved, environment)
    {
        diagnostics.push(missing_return_diagnostic(sources, block.span, context));
    }
}

fn check_block_return_statements(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> bool {
    for statement in &block.statements {
        check_statement_returns(
            sources,
            statement,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return true;
        }
    }
    if let Some(result) = &block.result {
        check_expression_for_nested_returns(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return expression_type(result, resolved, environment) == Type::Never;
    }

    false
}

fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_for_nested_returns(
                    sources,
                    expression,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            check_return_statement(
                sources,
                statement,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Stmt::Binding(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            let provenance = borrow_return_provenance_for_expression(
                &statement.initializer,
                &binding_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            borrow_provenance.define_binding(
                statement.name.clone(),
                type_contains_borrow_like(&binding_type, resolved),
                provenance,
            );
        }
        Stmt::Assignment(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.target,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.value,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(target_type) = environment.get(&identifier.name)
            {
                let provenance = borrow_return_provenance_for_expression(
                    &statement.value,
                    target_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
                borrow_provenance.define_binding(
                    identifier.name.clone(),
                    type_contains_borrow_like(target_type, resolved),
                    provenance,
                );
            }
        }
        Stmt::If(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            if !block_guarantees_return_or_never(&statement.then_block, resolved, &then_environment)
            {
                incoming.push(then_borrow_provenance);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                if !block_guarantees_return_or_never(else_block, resolved, &else_environment) {
                    incoming.push(else_borrow_provenance);
                }
            } else {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                statement,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            if !block_guarantees_return_or_never(&statement.then_block, resolved, &then_environment)
            {
                incoming.push(then_borrow_provenance);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                if !block_guarantees_return_or_never(else_block, resolved, &else_environment) {
                    incoming.push(else_borrow_provenance);
                }
            } else {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::Switch(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &statement.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                if !block_guarantees_return_or_never(&arm.body, resolved, &arm_environment) {
                    incoming.push(arm_borrow_provenance);
                }
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &wildcard_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                if !block_guarantees_return_or_never(
                    &wildcard_arm.body,
                    resolved,
                    &else_environment,
                ) {
                    incoming.push(else_borrow_provenance);
                }
            } else if !switch_statement_covers_all_variants(statement, resolved, environment) {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::While(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment = environment.clone();
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.start,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.end,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
        Stmt::Expression(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
    }
}

fn check_expression_for_nested_returns(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match expression {
        Expr::Propagate(expression) => {
            check_propagation(
                sources,
                expression.operator_span,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Catch(expression) => {
            check_catch_operand(
                sources,
                expression.catch_span,
                &expression.expression,
                resolved,
                environment,
                diagnostics,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            let mut catch_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.catch_block,
                context,
                resolved,
                diagnostics,
                &mut catch_environment,
                &mut catch_borrow_provenance,
                summaries,
            );
            if !block_guarantees_control_exit_or_never(
                &expression.catch_block,
                resolved,
                &catch_environment,
            ) {
                diagnostics.push(catch_block_fallthrough_diagnostic(
                    sources,
                    &expression.catch_block,
                ));
            }
        }
        Expr::Force(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Borrow(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Binary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.left,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.right,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Unary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.operand,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Call(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.callee,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            for argument in &expression.arguments {
                check_expression_for_nested_returns(
                    sources,
                    argument,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Index(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.index,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_for_nested_returns(
                    sources,
                    element,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_for_nested_returns(
                    sources,
                    &field.value,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression_for_nested_returns(
                        sources,
                        &part.expression,
                        context,
                        resolved,
                        diagnostics,
                        environment,
                        borrow_provenance,
                        summaries,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.value,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let present_borrow_provenance = borrow_provenance.clone();
            let mut fallback_environment = environment.clone();
            let mut fallback_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.fallback,
                context,
                resolved,
                diagnostics,
                &mut fallback_environment,
                &mut fallback_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![present_borrow_provenance];
            if !block_guarantees_control_exit_or_never(
                &expression.fallback,
                resolved,
                &fallback_environment,
            ) {
                incoming.push(fallback_borrow_provenance);
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Expr::If(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.condition,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            if let Some(else_block) = &expression.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::IfIs(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment =
                environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            check_block_return_statements(
                sources,
                &expression.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            if let Some(else_block) = &expression.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Match(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &expression.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &wildcard_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_body_result_return(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let expected = context.success_type();
    let actual = expression_type(expression, resolved, environment);

    if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
        return;
    }

    if expected == &Type::Void {
        if actual == Type::Void
            || actual == Type::Never
            || return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            )
        {
            return;
        }

        diagnostics.push(unexpected_body_result_diagnostic(
            sources, expression, context,
        ));
        return;
    }

    if expected.first_unsized_part().is_some() {
        return;
    }

    if return_expression_is_fallible_failure(expression, &actual, context, resolved, environment) {
        return;
    }

    if !is_expression_assignable(expected, expression, resolved, environment) {
        diagnostics.push(body_result_type_mismatch_diagnostic(
            sources, expression, expected, &actual, context,
        ));
        return;
    }

    check_borrow_return_provenance(
        sources,
        expression,
        &actual,
        context,
        resolved,
        environment,
        borrow_provenance,
        summaries,
        diagnostics,
    );

    if let Some(source) = implicit_non_copy_owned_value_source(expression, resolved, environment) {
        diagnostics.push(non_copy_struct_return_diagnostic(
            sources,
            expression,
            &source.source_name,
            &source.type_name,
            source.kind,
            context,
        ));
    }
}

fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let expected = context.success_type();
    if expected == &Type::Never {
        diagnostics.push(never_return_statement_diagnostic(
            sources, statement, context,
        ));
        return;
    }

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Unresolved(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            let actual = expression_type(expression, resolved, environment);
            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                return;
            }

            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = expression_type(expression, resolved, environment);
            if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
                return;
            }
            if expected.first_unsized_part().is_some() {
                return;
            }

            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                return;
            }

            if !is_expression_assignable(expected, expression, resolved, environment) {
                diagnostics.push(return_type_mismatch_diagnostic(
                    sources, expression, expected, &actual, context,
                ));
                return;
            }

            check_borrow_return_provenance(
                sources,
                expression,
                &actual,
                context,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                diagnostics,
            );

            if let Some(source) =
                implicit_non_copy_owned_value_source(expression, resolved, environment)
            {
                diagnostics.push(non_copy_struct_return_diagnostic(
                    sources,
                    expression,
                    &source.source_name,
                    &source.type_name,
                    source.kind,
                    context,
                ));
            }
        }
    }
}

fn check_borrow_return_provenance(
    sources: &SourceMap,
    expression: &Expr,
    ty: &Type,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(provenance) = borrow_return_provenance_for_expression(
        expression,
        ty,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) else {
        return;
    };
    let Some(source) = provenance.escaping_source() else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources, expression, source, context,
    ));
}

fn borrow_return_provenance_for_expression(
    expression: &Expr,
    ty: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(ty, resolved) {
        return None;
    }

    match unwrap_group(expression) {
        Expr::Borrow(_) => borrow_return_provenance_for_direct_borrow(expression, resolved),
        Expr::Identifier(identifier) => borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        ),
        Expr::StringLiteral(_) => Some(BorrowReturnProvenance::Static),
        Expr::StructLiteral(literal) => {
            let mut fields = BTreeMap::new();
            for field in &literal.fields {
                let field_type = expression_type(&field.value, resolved, environment);
                if let Some(field_provenance) = borrow_return_provenance_for_expression(
                    &field.value,
                    &field_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    fields.insert(field.name.clone(), field_provenance);
                }
            }
            (!fields.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
                fallback: None,
                fields,
                elements: BTreeMap::new(),
            })
        }
        Expr::Member(member) => borrow_return_provenance_for_member(
            member,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Index(index) => borrow_return_provenance_for_index(
            index,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::ArrayLiteral(literal) => {
            let mut elements = BTreeMap::new();
            for (index, element) in literal.elements.iter().enumerate() {
                let element_type = expression_type(element, resolved, environment);
                if let Some(element_provenance) = borrow_return_provenance_for_expression(
                    element,
                    &element_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    elements.insert(index, element_provenance);
                }
            }
            (!elements.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
                fallback: None,
                fields: BTreeMap::new(),
                elements,
            })
        }
        Expr::Call(call) if is_enum_variant_call(call, resolved) => {
            let mut provenance = None;
            for argument in &call.arguments {
                let argument_type = expression_type(argument, resolved, environment);
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_expression(
                        argument,
                        &argument_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        Expr::Call(call) => borrow_return_provenance_for_call(
            call,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Otherwise(expression) => {
            let mut provenance = borrow_return_provenance_for_expression(
                &expression.value,
                &expression_type(&expression.value, resolved, environment),
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    &expression.fallback,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::If(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::IfIs(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let then_environment = environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                &then_environment,
                &then_borrow_provenance,
                summaries,
            );
            merge_borrow_return_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::Match(expression) => {
            let mut provenance = None;
            for arm in &expression.arms {
                let arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &expression.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &arm.body,
                        resolved,
                        &arm_environment,
                        &arm_borrow_provenance,
                        summaries,
                    ),
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &wildcard_arm.body,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        _ => None,
    }
}

fn borrow_return_provenance_for_member(
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let object_type = expression_type(&member.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &member.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.field_provenance(&member.member))
}

fn borrow_return_provenance_for_index(
    index: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let object_type = expression_type(&index.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &index.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.element_provenance(index_literal_value(&index.index)))
}

fn index_literal_value(expression: &Expr) -> Option<usize> {
    integer_literal_expr_value(expression).and_then(|value| usize::try_from(value).ok())
}

fn borrow_return_provenance_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let signature = resolved_call_signature(resolved, call, environment)?;
    if let Some(declaration_span) = signature.declaration_span
        && let Some(summary) = summaries.get(&declaration_span)
    {
        return borrow_return_provenance_for_call_summary(
            summary,
            call,
            &signature,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    let mut provenance = None;
    if let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method_receiver_is_borrow(method)
        && let Some(member) = method_member_for_call(call)
        && let Some(receiver_provenance) = borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        )
    {
        merge_borrow_return_provenance(&mut provenance, Some(receiver_provenance));
    }

    for (argument, parameter) in call.arguments.iter().zip(&signature.signature.parameters) {
        let argument_type = expression_type(argument, resolved, environment);
        if !type_contains_borrow_like(&argument_type, resolved)
            && !type_expr_contains_borrow_like(
                &parameter.ty,
                resolved,
                &HashMap::new(),
                &mut HashSet::new(),
            )
        {
            continue;
        }

        merge_borrow_return_provenance(
            &mut provenance,
            borrow_return_provenance_for_borrowed_input(
                argument,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            ),
        );
    }

    provenance
}

fn borrow_return_provenance_for_call_summary(
    summary: &BorrowReturnProvenance,
    call: &crate::ast::CallExpr,
    signature: &super::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    match summary {
        BorrowReturnProvenance::Static => Some(BorrowReturnProvenance::Static),
        BorrowReturnProvenance::Escaping { .. } => None,
        BorrowReturnProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let mapped_fallback = fallback.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            let mut mapped_fields = BTreeMap::new();
            for (field, field_provenance) in fields {
                if let Some(mapped_field) = borrow_return_provenance_for_call_summary(
                    field_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_fields.insert(field.clone(), mapped_field);
                }
            }
            let mut mapped_elements = BTreeMap::new();
            for (index, element_provenance) in elements {
                if let Some(mapped_element) = borrow_return_provenance_for_call_summary(
                    element_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_elements.insert(*index, mapped_element);
                }
            }
            if mapped_fallback.is_none() && mapped_fields.is_empty() && mapped_elements.is_empty() {
                None
            } else {
                Some(BorrowReturnProvenance::Aggregate {
                    fallback: mapped_fallback.map(Box::new),
                    fields: mapped_fields,
                    elements: mapped_elements,
                })
            }
        }
        BorrowReturnProvenance::InputBorrow { sources } => {
            let mut provenance = None;
            for source in sources {
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_call_input(
                        source,
                        call,
                        signature,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
    }
}

fn borrow_return_provenance_for_call_input(
    source: &str,
    call: &crate::ast::CallExpr,
    signature: &super::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if signature.kind == super::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method.receiver.name == source
        && let Some(member) = method_member_for_call(call)
    {
        return borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    for (index, parameter) in signature.signature.parameters.iter().enumerate() {
        if parameter.name == source {
            return call.arguments.get(index).and_then(|argument| {
                borrow_return_provenance_for_borrowed_input(
                    argument,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
        }
    }

    None
}

fn merge_borrow_return_provenance(
    provenance: &mut Option<BorrowReturnProvenance>,
    next: Option<BorrowReturnProvenance>,
) {
    let Some(next) = next else {
        return;
    };
    if let Some(existing) = provenance {
        existing.merge(&next);
    } else {
        *provenance = Some(next);
    }
}

fn merge_borrow_return_boxed_provenance(
    provenance: &mut Option<Box<BorrowReturnProvenance>>,
    next: Option<BorrowReturnProvenance>,
) {
    let mut unboxed = provenance.take().map(|provenance| *provenance);
    merge_borrow_return_provenance(&mut unboxed, next);
    *provenance = unboxed.map(Box::new);
}

fn borrow_return_provenance_for_block_result(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let Some(result) = &block.result else {
        return None;
    };
    let mut result_environment = environment.clone();
    let mut result_borrow_provenance = borrow_provenance.clone();
    apply_borrow_return_statement_effects(
        block,
        resolved,
        &mut result_environment,
        &mut result_borrow_provenance,
        summaries,
    );
    let result_type = expression_type(result, resolved, &result_environment);
    borrow_return_provenance_for_expression(
        result,
        &result_type,
        resolved,
        &result_environment,
        &result_borrow_provenance,
        summaries,
    )
}

fn apply_borrow_return_statement_effects(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    for statement in &block.statements {
        apply_borrow_return_statement_effect(
            statement,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }
}

fn apply_borrow_return_statement_effect(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match statement {
        Stmt::Binding(statement) => {
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            let provenance = borrow_return_provenance_for_expression(
                &statement.initializer,
                &binding_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            borrow_provenance.define_binding(
                statement.name.clone(),
                type_contains_borrow_like(&binding_type, resolved),
                provenance,
            );
        }
        Stmt::Assignment(statement) => {
            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(target_type) = environment.get(&identifier.name)
            {
                let provenance = borrow_return_provenance_for_expression(
                    &statement.value,
                    target_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
                borrow_provenance.define_binding(
                    identifier.name.clone(),
                    type_contains_borrow_like(target_type, resolved),
                    provenance,
                );
            }
        }
        Stmt::If(statement) => {
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::IfIs(statement) => {
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                statement,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::Switch(statement) => {
            let mut incoming = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &statement.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                apply_borrow_return_statement_effects(
                    &arm.body,
                    resolved,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                incoming.push(arm_borrow_provenance);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut wildcard_environment = environment.clone();
                let mut wildcard_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    &wildcard_arm.body,
                    resolved,
                    &mut wildcard_environment,
                    &mut wildcard_borrow_provenance,
                    summaries,
                );
                incoming.push(wildcard_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        _ => {}
    }
}

fn define_if_is_payload_borrow_return_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding) = statement
        .payload
        .as_ref()
        .and_then(|payload| payload.binding())
    else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        &statement.expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

fn define_switch_arm_payload_borrow_return_binding(
    arm: &SwitchArm,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding) = arm.payload.as_ref().and_then(|payload| payload.binding()) else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        target_expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

fn define_payload_borrow_return_binding(
    binding: &SwitchPayloadBinding,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let Some(binding_type) = payload_environment.get(&binding.name) else {
        borrow_provenance.define_binding(binding.name.clone(), false, None);
        return;
    };
    let contains_borrow_like = type_contains_borrow_like(binding_type, resolved);
    let provenance = contains_borrow_like.then(|| {
        let target_type = expression_type(target_expression, resolved, source_environment);
        borrow_return_provenance_for_expression(
            target_expression,
            &target_type,
            resolved,
            source_environment,
            borrow_provenance,
            summaries,
        )
    });
    borrow_provenance.define_binding(
        binding.name.clone(),
        contains_borrow_like,
        provenance.flatten(),
    );
}

fn method_receiver_is_borrow(method: &crate::resolve::MethodSignature) -> bool {
    matches!(&method.receiver.ty, TypeExpr::Borrow(_))
}

fn borrow_return_provenance_for_borrowed_input(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let ty = expression_type(expression, resolved, environment);
    if type_contains_borrow_like(&ty, resolved) {
        return borrow_return_provenance_for_expression(
            expression,
            &ty,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    let Some(identifier) = expression_root_identifier(expression) else {
        return Some(BorrowReturnProvenance::escaping(
            "temporary expression".to_string(),
        ));
    };
    if environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        );
    }

    borrow_return_provenance_for_local_storage(identifier, resolved)
}

fn borrow_return_provenance_for_identifier(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
) -> Option<BorrowReturnProvenance> {
    if let Some(provenance) = borrow_provenance.get(&identifier.name) {
        return Some(provenance.clone());
    }

    if matches!(
        resolved.local_symbol_for_identifier(identifier)?.kind,
        LocalSymbolKind::Parameter
    ) && environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return Some(BorrowReturnProvenance::input_borrow(
            identifier.name.clone(),
        ));
    }

    None
}

fn borrow_return_provenance_for_direct_borrow(
    expression: &Expr,
    resolved: &ResolveOutput,
) -> Option<BorrowReturnProvenance> {
    let Expr::Borrow(borrow) = unwrap_group(expression) else {
        return None;
    };

    let source = match unwrap_group(&borrow.expression) {
        Expr::Identifier(identifier) => {
            borrow_return_provenance_for_local_storage(identifier, resolved)?
                .escaping_source()?
                .to_string()
        }
        _ => "temporary expression".to_string(),
    };

    Some(BorrowReturnProvenance::escaping(source))
}

fn borrow_return_provenance_for_local_storage(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
) -> Option<BorrowReturnProvenance> {
    let source = match resolved.local_symbol_for_identifier(identifier)?.kind {
        LocalSymbolKind::Parameter => format!("parameter `{}`", identifier.name),
        LocalSymbolKind::Binding(_) => format!("local binding `{}`", identifier.name),
        LocalSymbolKind::PatternPayload => format!("payload binding `{}`", identifier.name),
        LocalSymbolKind::CatchError => format!("catch binding `{}`", identifier.name),
        LocalSymbolKind::ForRange => format!("for-range binding `{}`", identifier.name),
    };

    Some(BorrowReturnProvenance::escaping(source))
}

fn type_contains_borrow_like(ty: &Type, resolved: &ResolveOutput) -> bool {
    type_contains_borrow_like_inner(ty, resolved, &mut HashSet::new())
}

fn type_contains_borrow_like_inner(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Str | Type::View { .. } => true,
        Type::Named(name) if name.starts_with('&') => true,
        Type::Named(name) => {
            type_symbol_contains_borrow_like(name, resolved, &HashMap::new(), resolving_names)
        }
        Type::Generic { name, arguments } => {
            let Some(symbol) = resolved.type_symbol_by_canonical_name(name) else {
                return false;
            };
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            type_symbol_contains_borrow_like(name, resolved, &substitutions, resolving_names)
        }
        Type::Array { element, .. } | Type::Optional(element) => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Fallible { success, error } => {
            type_contains_borrow_like_inner(success, resolved, resolving_names)
                || type_contains_borrow_like_inner(error, resolved, resolving_names)
        }
        Type::ArrayData { element } => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Error => true,
        Type::I32
        | Type::Primitive(_)
        | Type::StrData
        | Type::Void
        | Type::Never
        | Type::None
        | Type::Pointer(_)
        | Type::Parameter(_)
        | Type::Unresolved(_)
        | Type::Unknown => false,
    }
}

fn type_symbol_contains_borrow_like(
    canonical_name: &str,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    if !resolving_names.insert(canonical_name.to_string()) {
        return false;
    }

    let result = resolved
        .type_symbol_by_canonical_name(canonical_name)
        .is_some_and(|symbol| match symbol.kind {
            TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
                type_expr_contains_borrow_like(target, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Struct => symbol.fields.iter().any(|field| {
                type_expr_contains_borrow_like(&field.ty, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Enum => symbol.variants.iter().any(|variant| {
                variant.payload.iter().any(|payload| {
                    type_expr_contains_borrow_like(
                        &payload.ty,
                        resolved,
                        substitutions,
                        resolving_names,
                    )
                })
            }),
            TypeSymbolKind::Interface => false,
        });

    resolving_names.remove(canonical_name);
    result
}

fn type_expr_contains_borrow_like(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Borrow(_) => true,
        TypeExpr::View(view) => {
            type_expr_contains_borrow_like(&view.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_borrow_like(&array.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Optional(optional) => type_expr_contains_borrow_like(
            &optional.inner,
            resolved,
            substitutions,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_borrow_like(
                &fallible.success,
                resolved,
                substitutions,
                resolving_names,
            ) || type_expr_contains_borrow_like(
                &fallible.error,
                resolved,
                substitutions,
                resolving_names,
            )
        }
        TypeExpr::Pointer(_) => false,
        TypeExpr::Reference(reference) => {
            if reference.name == "error" {
                return true;
            }
            substitutions
                .get(&reference.name)
                .is_some_and(|ty| type_contains_borrow_like_inner(ty, resolved, resolving_names))
                || resolved
                    .type_symbol_by_reference_name(&reference.name)
                    .is_some_and(|symbol| {
                        type_symbol_contains_borrow_like(
                            &symbol.canonical_name,
                            resolved,
                            &HashMap::new(),
                            resolving_names,
                        )
                    })
        }
        TypeExpr::Generic(generic) => {
            if let Some(ty) = substitutions.get(&generic.name) {
                return type_contains_borrow_like_inner(ty, resolved, resolving_names);
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            let nested_substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().map(|argument| {
                    type_expr_to_type_with_substitutions(argument, resolved, None, substitutions)
                }))
                .collect();
            type_symbol_contains_borrow_like(
                &symbol.canonical_name,
                resolved,
                &nested_substitutions,
                resolving_names,
            )
        }
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn whole_identifier(expression: &Expr) -> Option<&crate::ast::IdentifierExpr> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Group(group) => whole_identifier(&group.expression),
        _ => None,
    }
}

fn expression_root_identifier(expression: &Expr) -> Option<&crate::ast::IdentifierExpr> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Member(member) => expression_root_identifier(&member.object),
        Expr::Index(index) => expression_root_identifier(&index.object),
        _ => None,
    }
}

fn return_expression_is_fallible_failure(
    expression: &Expr,
    actual: &Type,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let Type::Fallible { error, .. } = &context.declared_type else {
        return false;
    };

    !error.is_unknown_or_unresolved()
        && (is_expression_assignable(error, expression, resolved, environment)
            || super::operations::is_assignable(error, actual))
}

pub(super) fn block_guarantees_return(block: &Block) -> bool {
    for statement in &block.statements {
        if statement_guarantees_return(statement) {
            return true;
        }
    }

    block
        .result
        .as_deref()
        .is_some_and(expression_guarantees_return)
}

pub(super) fn block_guarantees_return_or_never(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let mut environment = environment.clone();
    for statement in &block.statements {
        if statement_guarantees_return_or_never(statement, resolved, &environment)
            || statement_evaluates_never_before_fallthrough(statement, resolved, &environment)
        {
            return true;
        }
        extend_terminal_lookahead_environment(statement, resolved, &mut environment);
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, &environment) == Type::Never)
}

pub(super) fn block_guarantees_control_exit_or_never(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let mut environment = environment.clone();
    for statement in &block.statements {
        if statement_guarantees_control_exit_or_never(statement, resolved, &environment)
            || statement_evaluates_never_before_fallthrough(statement, resolved, &environment)
        {
            return true;
        }
        extend_terminal_lookahead_environment(statement, resolved, &mut environment);
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, &environment) == Type::Never)
}

pub(super) fn statement_evaluates_never_before_fallthrough(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Binding(statement) => {
            expression_type(&statement.initializer, resolved, environment) == Type::Never
        }
        Stmt::Assignment(statement) => {
            expression_type(&statement.value, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => {
            expression_type(&statement.condition, resolved, environment) == Type::Never
        }
        Stmt::IfIs(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::Switch(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::ForRange(statement) => {
            expression_type(&statement.start, resolved, environment) == Type::Never
                || expression_type(&statement.end, resolved, environment) == Type::Never
        }
        Stmt::While(statement) => {
            expression_type(&statement.condition, resolved, environment) == Type::Never
        }
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Return(_)
        | Stmt::Loop(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

pub(super) fn extend_terminal_lookahead_environment(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
) {
    let Stmt::Binding(statement) = statement else {
        return;
    };
    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type == Type::Never {
        return;
    }
    let binding_type = continuing_binding_type(statement, initializer_type, resolved, environment);
    environment.define_binding(
        statement.name.clone(),
        binding_type,
        binding_kind_is_mutable(statement.kind),
    );
}

pub(super) fn statement_guarantees_control_exit_or_never(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_control_exit_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_control_exit_or_never(else_block, resolved, environment)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_control_exit_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_control_exit_or_never(else_block, resolved, environment)
        }),
        Stmt::Switch(statement) => {
            if !switch_arms_guarantee_control_exit_or_never(statement, resolved, environment) {
                return false;
            }

            statement.wildcard_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |wildcard_arm| {
                    block_guarantees_control_exit_or_never(
                        &wildcard_arm.body,
                        resolved,
                        environment,
                    )
                },
            )
        }
        Stmt::Loop(statement) => {
            block_guarantees_return_or_never(&statement.body, resolved, environment)
        }
        _ => statement_guarantees_return(statement),
    }
}

fn switch_arms_guarantee_control_exit_or_never(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    statement.arms.iter().all(|arm| {
        let arm_environment =
            environment_for_switch_arm(arm, &statement.expression, resolved, environment);
        block_guarantees_control_exit_or_never(&arm.body, resolved, &arm_environment)
    })
}

fn statement_guarantees_return_or_never(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match statement {
        Stmt::Expression(statement) => {
            expression_type(&statement.expression, resolved, environment) == Type::Never
        }
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_return_or_never(else_block, resolved, environment)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return_or_never(&statement.then_block, resolved, environment)
                && block_guarantees_return_or_never(else_block, resolved, environment)
        }),
        Stmt::Switch(statement) => {
            if !switch_arms_guarantee_return_or_never(statement, resolved, environment) {
                return false;
            }

            statement.wildcard_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |wildcard_arm| {
                    block_guarantees_return_or_never(&wildcard_arm.body, resolved, environment)
                },
            )
        }
        Stmt::Loop(statement) => {
            block_guarantees_return_or_never(&statement.body, resolved, environment)
        }
        _ => statement_guarantees_return(statement),
    }
}

fn switch_arms_guarantee_return_or_never(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    statement.arms.iter().all(|arm| {
        let arm_environment =
            environment_for_switch_arm(arm, &statement.expression, resolved, environment);
        block_guarantees_return_or_never(&arm.body, resolved, &arm_environment)
    })
}

fn expression_guarantees_return(expression: &Expr) -> bool {
    match expression {
        Expr::If(expression) => expression.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&expression.then_block) && block_guarantees_return(else_block)
        }),
        Expr::IfIs(expression) => expression.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&expression.then_block) && block_guarantees_return(else_block)
        }),
        Expr::Match(expression) => expression
            .wildcard_arm
            .as_ref()
            .is_some_and(|wildcard_arm| {
                expression
                    .arms
                    .iter()
                    .all(|arm| block_guarantees_return(&arm.body))
                    && block_guarantees_return(&wildcard_arm.body)
            }),
        Expr::Group(group) => expression_guarantees_return(&group.expression),
        _ => false,
    }
}

fn statement_guarantees_return(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::Switch(statement) => statement.wildcard_arm.as_ref().is_some_and(|wildcard_arm| {
            statement
                .arms
                .iter()
                .all(|arm| block_guarantees_return(&arm.body))
                && block_guarantees_return(&wildcard_arm.body)
        }),
        Stmt::Loop(statement) => block_guarantees_return(&statement.body),
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_)
        | Stmt::Expression(_) => false,
    }
}
