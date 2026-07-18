use super::diagnostics::{
    for_range_bound_type_mismatch_diagnostic, if_condition_type_mismatch_diagnostic,
    optional_if_let_non_optional_diagnostic, optional_if_let_projection_binding_kind_diagnostic,
    optional_while_let_non_optional_diagnostic, optional_while_let_projection_deferred_diagnostic,
    while_condition_type_mismatch_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::numeric::is_integer_type;
use super::operations::{integer_operands_match, is_bool_type};
use super::optional_projections::{
    optional_borrow_projection_type, optional_projection_binding_kind_is_allowed,
};
use crate::ast::{ForRangeStmt, IfLetStmt, IfStmt, WhileLetStmt, WhileStmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_if_let_initializer(
    sources: &SourceMap,
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if let Some(projection) =
        optional_borrow_projection_type(&statement.initializer, resolved, environment)
    {
        if !optional_projection_binding_kind_is_allowed(statement.kind, projection.is_readwrite) {
            diagnostics.push(optional_if_let_projection_binding_kind_diagnostic(
                sources,
                statement,
                projection.is_readwrite,
            ));
        }
        return;
    }

    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type.is_unknown() || matches!(initializer_type, Type::Optional(_)) {
        return;
    }

    diagnostics.push(optional_if_let_non_optional_diagnostic(
        sources,
        statement,
        &initializer_type,
    ));
}

pub(super) fn check_while_condition(
    sources: &SourceMap,
    statement: &WhileStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let condition_type = expression_type(&statement.condition, resolved, environment);
    if condition_type.is_unknown_or_unresolved() || is_bool_type(&condition_type) {
        return;
    }

    diagnostics.push(while_condition_type_mismatch_diagnostic(
        sources,
        &statement.condition,
        &condition_type,
    ));
}

pub(super) fn check_while_let_initializer(
    sources: &SourceMap,
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if optional_borrow_projection_type(&statement.initializer, resolved, environment).is_some() {
        diagnostics.push(optional_while_let_projection_deferred_diagnostic(
            sources, statement,
        ));
        return;
    }

    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type.is_unknown() || matches!(initializer_type, Type::Optional(_)) {
        return;
    }

    diagnostics.push(optional_while_let_non_optional_diagnostic(
        sources,
        statement,
        &initializer_type,
    ));
}

pub(super) fn check_for_range_bounds(
    sources: &SourceMap,
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let start_type = expression_type(&statement.start, resolved, environment);
    let end_type = expression_type(&statement.end, resolved, environment);

    if start_type.is_unknown_or_unresolved() || end_type.is_unknown_or_unresolved() {
        return;
    }

    if is_integer_type(&start_type)
        && is_integer_type(&end_type)
        && integer_operands_match(
            &start_type,
            &statement.start,
            &end_type,
            &statement.end,
            resolved,
            environment,
        )
    {
        return;
    }

    diagnostics.push(for_range_bound_type_mismatch_diagnostic(
        sources,
        statement,
        &start_type,
        &end_type,
    ));
}

pub(super) fn check_if_condition(
    sources: &SourceMap,
    statement: &IfStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let condition_type = expression_type(&statement.condition, resolved, environment);
    if condition_type.is_unknown_or_unresolved() || is_bool_type(&condition_type) {
        return;
    }

    diagnostics.push(if_condition_type_mismatch_diagnostic(
        sources,
        &statement.condition,
        &condition_type,
    ));
}
