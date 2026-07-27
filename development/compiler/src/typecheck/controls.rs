use super::diagnostics::{
    for_range_bound_type_mismatch_diagnostic, if_condition_type_mismatch_diagnostic,
    while_condition_type_mismatch_diagnostic,
};
use super::expressions::expression_type;
use super::model::TypeEnvironment;
use super::numeric::is_integer_type;
use super::operations::{integer_operands_match, is_bool_type};
use crate::ast::{ForRangeStmt, IfStmt, WhileStmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

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
