use super::*;

pub(super) fn unsupported_value_control_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "native lowering can only lower value control expressions with `else`, a final expression in every branch, and supported leading statements",
    )]
}

pub(super) fn unsupported_aggregate_call_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "native lowering cannot lower discarded aggregate call statement",
    )]
}

pub(super) fn unsupported_aggregate_literal_statement_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "native lowering cannot lower discarded aggregate literal statement",
    )]
}

pub(super) fn unsupported_catch_block_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        "native lowering can only lower catch blocks containing leading scalar local bindings, scalar assignments, or effect-only call statements followed by `return`",
    )]
}

pub(super) fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as an `i32` runtime value",
    )]
}

pub(super) fn unsupported_aggregate_member_field_access_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "native lowering cannot lower this aggregate member field access",
    )]
}

pub(super) fn unsupported_u8_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as a `u8` runtime value",
    )]
}

pub(super) fn unsupported_usize_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as a `usize` runtime value",
    )]
}

pub(super) fn unsupported_str_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as an `&str` runtime value",
    )]
}

pub(super) fn unsupported_slice_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as a slice runtime value",
    )]
}

pub(super) fn unavailable_call_target_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering requires a statically resolved function or method target for this call",
    )]
}

pub(super) fn unsupported_borrow_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this expression as the required borrow value",
    )]
}

pub(super) fn unsupported_scalar_call_value_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "native lowering cannot materialize this call without a scalar destination",
    )]
}

pub(super) fn unsupported_bool_expression_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "native lowering cannot materialize this expression as a `bool` runtime value",
    )]
}
