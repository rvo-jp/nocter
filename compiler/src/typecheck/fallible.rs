use super::diagnostics::{
    fail_in_non_fallible_context_diagnostic, fail_type_mismatch_diagnostic,
    try_error_type_mismatch_diagnostic, try_in_non_fallible_context_diagnostic,
    try_on_non_fallible_diagnostic,
};
use super::expressions::expression_type;
use super::model::{ReturnContext, Type, TypeEnvironment, same_known_type};
use super::operations::is_expression_assignable;
use crate::ast::{Expr, FailStmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};

pub(super) fn check_fail_statement(
    sources: &SourceMap,
    statement: &FailStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Type::Fallible {
        error: expected, ..
    } = &context.declared_type
    else {
        if !context.declared_type.is_unknown_or_unresolved() {
            diagnostics.push(fail_in_non_fallible_context_diagnostic(
                sources, statement, context,
            ));
        }
        return;
    };

    let actual = expression_type(&statement.expression, resolved, environment);
    if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
        return;
    }

    if !is_expression_assignable(expected, &statement.expression, resolved, environment) {
        diagnostics.push(fail_type_mismatch_diagnostic(
            sources, statement, expected, &actual, context,
        ));
    }
}

pub(super) fn check_try_propagation(
    sources: &SourceMap,
    try_span: ByteSpan,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let attempted = expression_type(expression, resolved, environment);
    let Type::Fallible {
        error: attempted_error,
        ..
    } = attempted
    else {
        if !attempted.is_unknown() {
            diagnostics.push(try_on_non_fallible_diagnostic(
                sources, try_span, &attempted,
            ));
        }
        return;
    };

    let Type::Fallible {
        error: current_error,
        ..
    } = &context.declared_type
    else {
        diagnostics.push(try_in_non_fallible_context_diagnostic(
            sources,
            try_span,
            context,
            &attempted_error,
        ));
        return;
    };

    if !same_known_type(current_error, &attempted_error) {
        diagnostics.push(try_error_type_mismatch_diagnostic(
            sources,
            try_span,
            context,
            current_error,
            &attempted_error,
        ));
    }
}

pub(super) fn check_try_catch_operand(
    sources: &SourceMap,
    try_span: ByteSpan,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let attempted = expression_type(expression, resolved, environment);
    if attempted.is_unknown() || matches!(attempted, Type::Fallible { .. }) {
        return;
    }

    diagnostics.push(try_on_non_fallible_diagnostic(
        sources, try_span, &attempted,
    ));
}
