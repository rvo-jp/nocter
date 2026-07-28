use super::{
    Block, ByteSpan, Diagnostic, ReturnContext, SourceMap, Type, add_declared_return_note,
};

pub(in crate::typecheck) fn propagation_on_non_propagatable_diagnostic(
    sources: &SourceMap,
    operator_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0330",
        format!(
            "postfix `?` requires a fallible or optional expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(operator_span).ok().map(Box::new);
    diagnostic.help =
        Some("use postfix `?` only on a value whose type is `T!` or `T?`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn catch_on_non_fallible_diagnostic(
    sources: &SourceMap,
    catch_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0330",
        format!(
            "`catch` requires a fallible expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(catch_span).ok().map(Box::new);
    diagnostic.help = Some("use `catch` only on `T!`; use `?` or `otherwise` for `T?`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn catch_block_fallthrough_diagnostic(
    sources: &SourceMap,
    block: &Block,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0337",
        "`catch` block may reach the end without leaving the current control path",
    );
    diagnostic.primary_span = sources.span_to_json(block.span).ok().map(Box::new);
    diagnostic.help = Some(
        "end the `catch` block with `return`, `break`, `continue`, or a `never` expression"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn fallible_propagation_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    operator_span: ByteSpan,
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
    diagnostic.primary_span = sources.span_to_json(operator_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help =
        Some("add `catch error { ... }` or make the current callable return `T!`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn optional_propagation_in_non_optional_context_diagnostic(
    sources: &SourceMap,
    operator_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0335",
        format!(
            "postfix `?` would propagate `none`, but {} does not return an optional value",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(operator_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("handle `none` with `?` or `otherwise`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn force_on_non_unwrappable_diagnostic(
    sources: &SourceMap,
    operator_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0336",
        format!(
            "postfix `!` requires a fallible or optional expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(operator_span).ok().map(Box::new);
    diagnostic.help =
        Some("use postfix `!` only on a value whose type is `T!` or `T?`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn fallible_propagation_error_type_mismatch_diagnostic(
    sources: &SourceMap,
    operator_span: ByteSpan,
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
    diagnostic.primary_span = sources.span_to_json(operator_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("handle the failure with `catch`".to_string());
    diagnostic
}
