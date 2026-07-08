use super::bindings::{check_optional_let_else_statement, continuing_binding_type};
use super::diagnostics::{
    fallible_success_error_diagnostic, missing_return_diagnostic, missing_return_value_diagnostic,
    return_type_mismatch_diagnostic, unexpected_return_value_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_if_is_binding,
    environment_for_if_let_binding, environment_for_method, environment_for_parameters,
    environment_for_parameters_with_self_type, environment_for_pattern_conditional_arm,
    environment_for_switch_arm, environment_for_while_let_binding, impl_member_name,
    impl_self_type,
};
use super::expressions::expression_type;
use super::fallible::{check_catch_operand, check_propagation};
use super::model::{CallableKind, ReturnContext, Type, TypeEnvironment, binding_kind_is_mutable};
use super::operations::is_expression_assignable;
use super::type_expr::{type_expr_to_type, type_expr_to_type_with_self_type};
use crate::ast::{AstFile, Block, Expr, ImplDecl, ImplMember, Item, ReturnStmt, Stmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_return_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let context = ReturnContext::new(
                    CallableKind::Function(function.name.clone()),
                    type_expr_to_type(&function.return_type, resolved),
                    function.return_type.span(),
                );
                let mut environment =
                    environment_for_parameters(&function.parameters.parameters, resolved);
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    &function.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
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
    let self_type = impl_self_type(impl_, resolved);

    for member in &impl_.members {
        match member {
            ImplMember::Function(function) => {
                let context = ReturnContext::new(
                    CallableKind::AssociatedFunction(impl_member_name(impl_, &function.name)),
                    type_expr_to_type_with_self_type(
                        &function.return_type,
                        resolved,
                        Some(&self_type),
                    ),
                    function.return_type.span(),
                );
                let mut environment = environment_for_parameters_with_self_type(
                    &function.parameters.parameters,
                    resolved,
                    self_type.clone(),
                );
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    &function.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                );
            }
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let context = ReturnContext::new(
                    CallableKind::Method(impl_member_name(impl_, &method.name)),
                    type_expr_to_type_with_self_type(
                        &method.return_type,
                        resolved,
                        Some(&self_type),
                    ),
                    method.return_type.span(),
                );
                let mut environment = environment_for_method(method, resolved, self_type.clone());
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
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

    if success.as_ref() == &Type::Error {
        diagnostics.push(fallible_success_error_diagnostic(sources, context));
    }
}

fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    check_block_return_statements(sources, block, context, resolved, diagnostics, environment);

    if context.requires_explicit_return() && !block_guarantees_return(block) {
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
) {
    for statement in &block.statements {
        check_statement_returns(
            sources,
            statement,
            context,
            resolved,
            diagnostics,
            environment,
        );
    }
}

fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_for_nested_returns(
                    sources,
                    expression,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
            check_return_statement(
                sources,
                statement,
                context,
                resolved,
                diagnostics,
                environment,
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
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            if let Some(else_block) = &statement.else_block {
                check_optional_let_else_statement(
                    sources,
                    statement,
                    &initializer_type,
                    diagnostics,
                );
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type,
                binding_kind_is_mutable(statement.kind),
            );
        }
        Stmt::If(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut then_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
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
            );
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
        }
        Stmt::IfLet(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut then_environment =
                environment_for_if_let_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
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
            );
            for arm in &statement.arms {
                let mut arm_environment = environment_for_switch_arm(arm, resolved, environment);
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                );
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    &else_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
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
            );
            let mut body_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::WhileLet(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut body_environment =
                environment_for_while_let_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
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
            );
            check_expression_for_nested_returns(
                sources,
                &statement.end,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::Expression(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
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
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            check_block_return_statements(
                sources,
                &expression.catch_block,
                context,
                resolved,
                diagnostics,
                &mut catch_environment,
            );
        }
        Expr::Force(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
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
            );
            check_expression_for_nested_returns(
                sources,
                &expression.right,
                context,
                resolved,
                diagnostics,
                environment,
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
            );
            for argument in &expression.arguments {
                check_expression_for_nested_returns(
                    sources,
                    argument,
                    context,
                    resolved,
                    diagnostics,
                    environment,
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
            );
            check_expression_for_nested_returns(
                sources,
                &expression.index,
                context,
                resolved,
                diagnostics,
                environment,
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
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.value,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.default,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::PatternConditional(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.target,
                context,
                resolved,
                diagnostics,
                environment,
            );
            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_pattern_conditional_arm(arm, resolved, environment);
                check_expression_for_nested_returns(
                    sources,
                    &arm.expression,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                );
            }
            check_expression_for_nested_returns(
                sources,
                &expression.fallback,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let expected = context.success_type();

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Unresolved(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = expression_type(expression, resolved, environment);
            if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
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
            }
        }
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
    block
        .statements
        .last()
        .is_some_and(statement_guarantees_return)
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
        Stmt::IfLet(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
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
        Stmt::Binding(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::WhileLet(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Expression(_) => false,
    }
}
