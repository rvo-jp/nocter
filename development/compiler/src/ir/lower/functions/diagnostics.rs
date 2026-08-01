use super::*;

pub(super) fn unsupported_bare_return_diagnostic(
    diagnostic_code: &'static str,
    function_name: &str,
    return_label: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!("IR v0 cannot lower bare returns from {return_label} function `{function_name}`"),
    )]
}

pub(super) fn unsupported_return_diagnostic(
    diagnostic_code: &'static str,
    function_name: &str,
    return_label: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!("IR v0 cannot lower {return_label} returns from function `{function_name}`"),
    )]
}

pub(super) fn attach_primary_span_if_absent(
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
    span: ByteSpan,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.with_primary_span_if_absent(sources, span))
        .collect()
}

pub(super) fn unsupported_terminal_aggregate_if_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower terminal aggregate `if` branches in function `{function_name}` when both branches contain supported leading statements followed by aggregate returns or nested terminal aggregate `if` branches"
        ),
    )]
}

pub(super) fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn statement_allows_implicit_void_return(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Drop(_)
            | Stmt::ForRange(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
    )
}

pub(super) fn statement_is_import(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Import(_) | Stmt::FromImport(_))
}

pub(super) fn expression_is_none_literal(expression: &Expr) -> bool {
    matches!(unwrap_group(expression), Expr::NoneLiteral(_))
}

pub(super) fn unsupported_function_body_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{function_name}` bodies containing leading scalar local bindings, scalar assignments, or effect-only call statements followed by `return`"
        ),
    )]
}
