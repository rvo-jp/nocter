use super::{
    ByteSpan, Diagnostic, Expr, ReturnContext, ReturnStmt, SourceMap, Type,
    add_declared_return_note,
};

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

pub(in crate::typecheck) fn never_return_statement_diagnostic(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0314",
        format!(
            "`return` is not valid in {} returning `never`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help =
        Some("end the path with a call returning `never` instead of `return`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn fallible_success_error_diagnostic(
    sources: &SourceMap,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0334",
        format!(
            "{} returns `error!`, but a fallible success type cannot be `error`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(context.return_type_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "`return error_value` means failure in a fallible function; wrap the error in another type if it must be a success value"
            .to_string(),
    );
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
    diagnostic.help = Some(if expected == &Type::Never {
        "end the path with a call returning `never`".to_string()
    } else {
        format!(
            "add a `return` with a value of type `{}`",
            expected.display()
        )
    });
    diagnostic
}
