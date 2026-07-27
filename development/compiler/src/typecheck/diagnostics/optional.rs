use super::{Diagnostic, OtherwiseExpr, SourceMap, Type};

pub(in crate::typecheck) fn otherwise_non_optional_diagnostic(
    sources: &SourceMap,
    expression: &OtherwiseExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0396",
        format!(
            "`otherwise` requires an optional left operand, but the left operand has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.value.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("use a value whose type is `T?` on the left side of `otherwise`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn otherwise_fallback_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &OtherwiseExpr,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0397",
        format!(
            "`otherwise` fallback has type `{}`, but the optional payload has type `{}`",
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.fallback.span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "change the fallback block to produce `{}`",
        expected.display()
    ));
    diagnostic
}
