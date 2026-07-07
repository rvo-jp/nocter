use super::{ByteSpan, Diagnostic, ReturnContext, SourceMap, Type, add_declared_return_note};

pub(in crate::typecheck) fn try_on_non_fallible_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0330",
        format!(
            "fallible handling requires a fallible expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    diagnostic.help = Some(
        "remove postfix `?` or `catch`, or call a function whose return type is `T!`".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn try_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0331",
        format!(
            "postfix `?` would fail with `{}`, but {} is not fallible",
            attempted_error.display(),
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help =
        Some("add `catch error { ... }` or make the current callable return `T!`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn try_error_type_mismatch_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    current_error: &Type,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0332",
        format!(
            "postfix `?` would fail with `{}`, but {} fails with `{}`",
            attempted_error.display(),
            context.subject(),
            current_error.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("handle the failure with `catch`".to_string());
    diagnostic
}
