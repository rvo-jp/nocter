use super::{Diagnostic, InterpolatedStringExpression, SourceMap, Type};

pub(in crate::typecheck) fn interpolated_string_part_type_unsupported_diagnostic(
    sources: &SourceMap,
    part: &InterpolatedStringExpression,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0379",
        format!(
            "string interpolation expression has type `{}`, but interpolation currently supports `str`, `String`, integer, and `bool` values",
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
