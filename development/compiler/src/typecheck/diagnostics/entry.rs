use super::{ByteSpan, Diagnostic, SourceMap};
use crate::entry::DEFAULT_ENTRY_NAME;

pub(in crate::typecheck) fn missing_entry_function_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        format!(
            "executable root file must define entry function `{}`",
            DEFAULT_ENTRY_NAME
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "add `func {0}(): i32! {{ ... }}`, `func {0}(): i32 {{ ... }}`, `func {0}(): usize! {{ ... }}`, `func {0}(): usize {{ ... }}`, `func {0}(): void! {{ ... }}`, or `func {0}(): void {{ ... }}`",
        DEFAULT_ENTRY_NAME
    ));
    diagnostic
}

pub(in crate::typecheck) fn invalid_entry_function_diagnostic(
    sources: &SourceMap,
    function_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        format!(
            "entry function `{}` must have no type parameters, no value parameters, and return `i32!`, `i32`, `usize!`, `usize`, `void!`, or `void`",
            DEFAULT_ENTRY_NAME
        ),
    );
    diagnostic.primary_span = sources.span_to_json(function_span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "use `func {0}(): i32!`, `func {0}(): i32`, `func {0}(): usize!`, `func {0}(): usize`, `func {0}(): void!`, or `func {0}(): void`",
        DEFAULT_ENTRY_NAME
    ));
    diagnostic
}
