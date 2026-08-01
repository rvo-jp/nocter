use super::*;

pub(super) fn unsupported_value_control_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 can only lower value control expressions with `else`, a final expression in every branch, and supported leading statements",
    )]
}

pub(super) fn unsupported_aggregate_call_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 cannot lower discarded aggregate call statement",
    )]
}

pub(super) fn unsupported_aggregate_literal_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 cannot lower discarded aggregate literal statement",
    )]
}

pub(super) fn unsupported_catch_block_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "IR v0 can only lower catch blocks containing leading scalar local bindings, scalar assignments, or effect-only call statements followed by `return`",
    )]
}

pub(super) fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, arithmetic or shift expressions, and direct tail calls",
    )]
}

pub(super) fn unsupported_aggregate_member_field_access_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 cannot lower this aggregate member field access",
    )]
}

pub(super) fn unsupported_u8_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower u8 literals, parameters, locals, direct tail calls, and indexing into `&str`, `&[u8]`, or `&+[u8]`",
    )]
}

pub(super) fn unsupported_usize_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower usize literals, parameters, locals, arithmetic or shift expressions, slice indexing, len calls, and direct tail calls",
    )]
}

pub(super) fn unsupported_str_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower string literals and `&str` parameters as `&str` values",
    )]
}

pub(super) fn unsupported_slice_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower slice parameters and locals as slice values",
    )]
}

pub(super) fn unsupported_non_tail_call_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower function calls in direct tail return position",
    )]
}

pub(super) fn unsupported_bool_expression_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool literals, bool locals, bool operators, i32, u8, usize comparisons, and bool equality/inequality over lowerable bool values",
    )]
}
