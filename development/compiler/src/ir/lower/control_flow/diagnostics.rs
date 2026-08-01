use super::*;

pub(super) fn unsupported_terminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
    return_type: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower terminal `if` statements for {subject} when both branches contain only supported binding, assignment, explicit `drop`, or effect-only call statements followed by returns or nested terminal `if` branches returning `{return_type}`"
        ),
    )]
}

pub(super) fn unsupported_nonterminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower non-terminal `if`/`while`/`loop` statements for {subject} when branches/bodies contain supported local bindings, branch/body-local assignments, outer scalar/view/aggregate local assignments, explicit aggregate drops, effect-only call statements, returns, or nested non-terminal `if`/`while`/`loop` statements"
        ),
    )]
}

pub(super) fn unsupported_control_flow_condition_move_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 cannot lower control-flow conditions that explicitly move aggregate values",
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
