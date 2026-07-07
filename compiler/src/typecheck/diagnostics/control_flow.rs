use super::*;

pub(in crate::typecheck) fn if_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0346",
        format!(
            "`if` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

pub(in crate::typecheck) fn while_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0357",
        format!(
            "`while` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

pub(in crate::typecheck) fn loop_control_outside_loop_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    keyword: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0359",
        format!("`{keyword}` can only be used inside a loop"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!("move `{keyword}` inside a loop body"));
    diagnostic
}
pub(in crate::typecheck) fn for_range_bound_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &ForRangeStmt,
    start_type: &Type,
    end_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0360",
        format!(
            "`for` range bounds have types `{}` and `{}`, but range `for` requires matching integer bounds",
            start_type.display(),
            end_type.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.range_span)
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(statement.start.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range start has type `{}`", start_type.display()),
            span: Some(span),
        });
    }
    if let Ok(span) = sources.span_to_json(statement.end.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range end has type `{}`", end_type.display()),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("use integer bounds with the same type, or an integer literal that fits the other bound type".to_string());
    diagnostic
}
