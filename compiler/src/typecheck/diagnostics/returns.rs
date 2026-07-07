use super::*;

pub(in crate::typecheck) fn missing_return_value_diagnostic(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0310",
        format!(
            "`return` has no value, but {} returns `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

pub(in crate::typecheck) fn unexpected_return_value_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0311",
        format!(
            "`return` has a value, but {} returns `void`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("remove the returned value or change the return type".to_string());
    diagnostic
}

pub(in crate::typecheck) fn return_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0312",
        format!(
            "`return` value has type `{}`, but {} returns `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

pub(in crate::typecheck) fn fail_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0333",
        format!(
            "`fail` is used in {}, but its return type is not fallible",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("use `fail` only inside a function returning `T!`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn fail_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0334",
        format!(
            "`fail` value has type `{}`, but {} fails with `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "fail with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

pub(in crate::typecheck) fn missing_return_diagnostic(
    sources: &SourceMap,
    block_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0313",
        format!(
            "{} may reach the end without returning `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(block_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "add a `return` with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}
