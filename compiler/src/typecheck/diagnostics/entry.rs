use super::{ByteSpan, Diagnostic, SourceMap};

pub(in crate::typecheck) fn missing_entry_function_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    entry_name: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        format!("executable root file must define entry function `{entry_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "add `func {entry_name}(): i32! {{ ... }}`, `func {entry_name}(): i32 {{ ... }}`, `func {entry_name}(): usize! {{ ... }}`, `func {entry_name}(): usize {{ ... }}`, `func {entry_name}(): void! {{ ... }}`, or `func {entry_name}(): void {{ ... }}`"
    ));
    diagnostic
}

pub(in crate::typecheck) fn invalid_entry_function_diagnostic(
    sources: &SourceMap,
    function_span: ByteSpan,
    entry_name: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        format!(
            "entry function `{entry_name}` must have no parameters and return `i32!`, `i32`, `usize!`, `usize`, `void!`, or `void` in v0"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(function_span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "use `func {entry_name}(): i32!`, `func {entry_name}(): i32`, `func {entry_name}(): usize!`, `func {entry_name}(): usize`, `func {entry_name}(): void!`, or `func {entry_name}(): void`"
    ));
    diagnostic
}
