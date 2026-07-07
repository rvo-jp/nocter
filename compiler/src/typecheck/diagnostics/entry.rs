use super::*;

pub(in crate::typecheck) fn missing_program_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        "executable root file must define exactly one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "add `program(): i32! { ... }`, `program(): i32 { ... }`, or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn main_is_not_entry_diagnostic(
    sources: &SourceMap,
    function: &FunctionDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0301",
        "`func main` is an ordinary function; Nocter executable entry uses `program`",
    );
    diagnostic.primary_span = sources.span_to_json(function.name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "replace the entry declaration with `program(): i32! { ... }`, `program(): i32 { ... }`, or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn duplicate_program_diagnostic(
    sources: &SourceMap,
    first_span: ByteSpan,
    second_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0302",
        "executable root file must not define more than one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(second_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first `program` entry is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one top-level `program` declaration".to_string());
    diagnostic
}

pub(in crate::typecheck) fn invalid_program_return_type_diagnostic(
    sources: &SourceMap,
    return_type_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        "`program` return type must be `i32!`, `i32`, or `void` in v0",
    );
    diagnostic.primary_span = sources.span_to_json(return_type_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `program(): i32!` for a fallible entry point, `program(): i32` for an infallible exit status, or `program(): void` for status 0"
            .to_string(),
    );
    diagnostic
}
