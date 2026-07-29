use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_call_signature, resolved_method_for_call};
use super::copyability::implicit_non_copy_struct_value_source;
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
use super::operations::is_expression_assignable;
use super::type_expr::{type_expr_to_type_in_environment, type_expr_to_type_with_substitutions};
use super::variants::{is_enum_variant_call, switch_statement_covers_all_variants};
use crate::ast::{
    AstFile, Block, Expr, ImplDecl, ImplMember, InterpolatedStringPart, Item, ReturnStmt, Stmt,
    TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{LocalSymbolKind, ResolveOutput, TypeSymbolKind};
use crate::source::SourceMap;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
struct BorrowReturnEnvironment {
    bindings: HashMap<String, BorrowReturnProvenance>,
}

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
                    .or_insert_with(|| provenance.clone());
            }
        }
        self.bindings = joined;
    }
}

#[derive(Debug, Clone)]
struct BorrowReturnProvenance {
    source: String,
}

pub(super) fn check_return_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_return_types(sources, impl_, resolved, diagnostics);
            }
            _ => {}
        }
    }
}

fn check_impl_member_return_types(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
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
            );
            check_expression_for_nested_returns(
                sources,
                &statement.value,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
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
            );
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
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
            );
            let mut incoming = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                );
                if !block_guarantees_return_or_never(&arm.body, resolved, &arm_environment) {
                    incoming.push(arm_borrow_provenance);
                }
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &else_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                );
                if !block_guarantees_return_or_never(&else_arm.body, resolved, &else_environment) {
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
            );
            check_expression_for_nested_returns(
                sources,
                &statement.end,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
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
            );
            check_expression_for_nested_returns(
                sources,
                &expression.right,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
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
            );
            check_expression_for_nested_returns(
                sources,
                &expression.index,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
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
            );
            let mut then_environment =
                environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
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
            );
            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                );
            }
            if let Some(else_arm) = &expression.else_arm {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &else_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
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
        diagnostics,
    );

    if let Some((source_name, type_name)) =
        implicit_non_copy_struct_value_source(expression, resolved, environment)
    {
        diagnostics.push(non_copy_struct_return_diagnostic(
            sources,
            expression,
            &source_name,
            &type_name,
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
                diagnostics,
            );

            if let Some((source_name, type_name)) =
                implicit_non_copy_struct_value_source(expression, resolved, environment)
            {
                diagnostics.push(non_copy_struct_return_diagnostic(
                    sources,
                    expression,
                    &source_name,
                    &type_name,
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
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(provenance) = borrow_return_provenance_for_expression(
        expression,
        ty,
        resolved,
        environment,
        borrow_provenance,
    ) else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources,
        expression,
        &provenance.source,
        context,
    ));
}

fn borrow_return_provenance_for_expression(
    expression: &Expr,
    ty: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(ty, resolved) {
        return None;
    }

    match unwrap_group(expression) {
        Expr::Borrow(_) => borrow_return_provenance_for_direct_borrow(expression, resolved),
        Expr::Identifier(identifier) => borrow_provenance.get(&identifier.name).cloned(),
        Expr::StructLiteral(literal) => {
            for field in &literal.fields {
                let field_type = expression_type(&field.value, resolved, environment);
                if let Some(provenance) = borrow_return_provenance_for_expression(
                    &field.value,
                    &field_type,
                    resolved,
                    environment,
                    borrow_provenance,
                ) {
                    return Some(provenance);
                }
            }
            None
        }
        Expr::ArrayLiteral(literal) => {
            for element in &literal.elements {
                let element_type = expression_type(element, resolved, environment);
                if let Some(provenance) = borrow_return_provenance_for_expression(
                    element,
                    &element_type,
                    resolved,
                    environment,
                    borrow_provenance,
                ) {
                    return Some(provenance);
                }
            }
            None
        }
        Expr::Call(call) if is_enum_variant_call(call, resolved) => {
            for argument in &call.arguments {
                let argument_type = expression_type(argument, resolved, environment);
                if let Some(provenance) = borrow_return_provenance_for_expression(
                    argument,
                    &argument_type,
                    resolved,
                    environment,
                    borrow_provenance,
                ) {
                    return Some(provenance);
                }
            }
            None
        }
        Expr::Call(call) => {
            borrow_return_provenance_for_call(call, resolved, environment, borrow_provenance)
        }
        _ => None,
    }
}

fn borrow_return_provenance_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
) -> Option<BorrowReturnProvenance> {
    if let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method_receiver_is_borrow(method)
        && let Some(member) = method_member_for_call(call)
        && let Some(provenance) = borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
        )
    {
        return Some(provenance);
    }

    let signature = resolved_call_signature(resolved, call, environment)?;
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

        if let Some(provenance) = borrow_return_provenance_for_borrowed_input(
            argument,
            resolved,
            environment,
            borrow_provenance,
        ) {
            return Some(provenance);
        }
    }

    None
}

fn method_receiver_is_borrow(method: &crate::resolve::MethodSignature) -> bool {
    matches!(&method.receiver.ty, TypeExpr::Borrow(_))
}

fn borrow_return_provenance_for_borrowed_input(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
) -> Option<BorrowReturnProvenance> {
    let ty = expression_type(expression, resolved, environment);
    if type_contains_borrow_like(&ty, resolved) {
        return borrow_return_provenance_for_expression(
            expression,
            &ty,
            resolved,
            environment,
            borrow_provenance,
        );
    }

    let Some(identifier) = expression_root_identifier(expression) else {
        return Some(BorrowReturnProvenance {
            source: "temporary expression".to_string(),
        });
    };
    if environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return borrow_provenance.get(&identifier.name).cloned();
    }

    borrow_return_provenance_for_local_storage(identifier, resolved)
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
            borrow_return_provenance_for_local_storage(identifier, resolved)?.source
        }
        _ => "temporary expression".to_string(),
    };

    Some(BorrowReturnProvenance { source })
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

    Some(BorrowReturnProvenance { source })
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
        Type::I32
        | Type::Primitive(_)
        | Type::StrData
        | Type::Error
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
    for statement in &block.statements {
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return true;
        }
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, environment) == Type::Never)
}

pub(super) fn block_guarantees_control_exit_or_never(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    for statement in &block.statements {
        if statement_guarantees_control_exit_or_never(statement, resolved, environment) {
            return true;
        }
    }

    block
        .result
        .as_ref()
        .is_some_and(|result| expression_type(result, resolved, environment) == Type::Never)
}

fn statement_guarantees_control_exit_or_never(
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

            statement.else_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |else_arm| {
                    block_guarantees_control_exit_or_never(&else_arm.body, resolved, environment)
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

            statement.else_arm.as_ref().map_or_else(
                || switch_statement_covers_all_variants(statement, resolved, environment),
                |else_arm| block_guarantees_return_or_never(&else_arm.body, resolved, environment),
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
        Expr::Match(expression) => expression.else_arm.as_ref().is_some_and(|else_arm| {
            expression
                .arms
                .iter()
                .all(|arm| block_guarantees_return(&arm.body))
                && block_guarantees_return(&else_arm.body)
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
        Stmt::Switch(statement) => statement.else_arm.as_ref().is_some_and(|else_arm| {
            statement
                .arms
                .iter()
                .all(|arm| block_guarantees_return(&arm.body))
                && block_guarantees_return(&else_arm.body)
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
