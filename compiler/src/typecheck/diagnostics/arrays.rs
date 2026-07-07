use super::{Diagnostic, DiagnosticNote, Expr, IndexExpr, SourceMap, Type};

pub(in crate::typecheck) fn array_literal_element_type_mismatch_diagnostic(
    sources: &SourceMap,
    element: &Expr,
    element_type: &Type,
    first_element: &Expr,
    first_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0343",
        format!(
            "array literal element has type `{}`, but earlier elements have type `{}`",
            element_type.display(),
            first_type.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(element.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_element.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "array element type was inferred as `{}` here",
                first_type.display()
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some("make every array element have the same type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn index_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0344",
        format!(
            "index expression target has type `{}`, but indexing requires `[T; N]`, `[T]`, `[+T]`, or `str`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.object.span()).ok().map(Box::new);
    diagnostic.help = Some("index an array, view, or string value".to_string());
    diagnostic
}

pub(in crate::typecheck) fn index_value_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0345",
        format!(
            "index expression uses `{}` as the index, but indexes must be integer values",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.index_span).ok().map(Box::new);
    diagnostic.help = Some("use an integer value as the index".to_string());
    diagnostic
}
