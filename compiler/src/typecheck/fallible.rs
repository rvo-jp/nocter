use super::diagnostics::{
    catch_on_non_fallible_diagnostic, fallible_propagation_error_type_mismatch_diagnostic,
    fallible_propagation_in_non_fallible_context_diagnostic, force_on_non_unwrappable_diagnostic,
    optional_propagation_in_non_optional_context_diagnostic,
    propagation_on_non_propagatable_diagnostic,
};
use super::expressions::expression_type;
use super::model::{ReturnContext, Type, TypeEnvironment, same_known_type};
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};

pub(super) fn check_propagation(
    sources: &SourceMap,
    operator_span: ByteSpan,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let attempted = expression_type(expression, resolved, environment);
    match attempted {
        Type::Fallible {
            error: attempted_error,
            ..
        } => check_fallible_propagation(
            sources,
            operator_span,
            context,
            &attempted_error,
            diagnostics,
        ),
        Type::Optional(_) => {
            check_optional_propagation(sources, operator_span, context, diagnostics)
        }
        Type::Unknown | Type::Unresolved(_) => {}
        _ => diagnostics.push(propagation_on_non_propagatable_diagnostic(
            sources,
            operator_span,
            &attempted,
        )),
    }
}

fn check_fallible_propagation(
    sources: &SourceMap,
    operator_span: ByteSpan,
    context: &ReturnContext,
    attempted_error: &Type,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Type::Fallible {
        error: current_error,
        ..
    } = &context.declared_type
    else {
        diagnostics.push(fallible_propagation_in_non_fallible_context_diagnostic(
            sources,
            operator_span,
            context,
            attempted_error,
        ));
        return;
    };

    if !same_known_type(current_error, attempted_error) {
        diagnostics.push(fallible_propagation_error_type_mismatch_diagnostic(
            sources,
            operator_span,
            context,
            current_error,
            attempted_error,
        ));
    }
}

fn check_optional_propagation(
    sources: &SourceMap,
    operator_span: ByteSpan,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(context.success_type(), Type::Optional(_)) {
        return;
    }

    diagnostics.push(optional_propagation_in_non_optional_context_diagnostic(
        sources,
        operator_span,
        context,
    ));
}

pub(super) fn check_catch_operand(
    sources: &SourceMap,
    catch_span: ByteSpan,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let attempted = expression_type(expression, resolved, environment);
    if attempted.is_unknown() || matches!(attempted, Type::Fallible { .. }) {
        return;
    }

    diagnostics.push(catch_on_non_fallible_diagnostic(
        sources, catch_span, &attempted,
    ));
}

pub(super) fn check_force_unwrap_operand(
    sources: &SourceMap,
    bang_span: ByteSpan,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let attempted = expression_type(expression, resolved, environment);
    if attempted.is_unknown_or_unresolved()
        || matches!(attempted, Type::Fallible { .. } | Type::Optional(_))
    {
        return;
    }

    diagnostics.push(force_on_non_unwrappable_diagnostic(
        sources, bang_span, &attempted,
    ));
}
