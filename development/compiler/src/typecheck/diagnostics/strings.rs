use super::{Diagnostic, InterpolatedStringExpression, SourceMap, Type};

pub(in crate::typecheck) fn interpolation_runtime_unavailable_diagnostic(
    sources: &SourceMap,
    expression: &crate::ast::InterpolatedStringExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0440",
        "string interpolation runtime is unavailable in the active Nocter home",
    );
    diagnostic.primary_span = sources.span_to_json(expression.span).ok().map(Box::new);
    diagnostic.help = Some(
        "restore the trusted std/string and std/fmt declarations required by this compiler"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn interpolated_string_part_type_unsupported_diagnostic(
    sources: &SourceMap,
    part: &InterpolatedStringExpression,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0379",
        format!(
            "string interpolation expression has type `{}`, but v0.3.0 Phase 3 supports `&str`, `String`, `i32`, `u8`, `usize`, and `bool` values",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(part.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("convert the value to a supported string interpolation input first".to_string());
    diagnostic
}
