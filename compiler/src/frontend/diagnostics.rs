use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::path::PathBuf;

pub(super) fn relative_import_without_file_path_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        "relative import cannot be resolved because the importing source has no file path",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help =
        Some("load the root source from a file before resolving relative imports".to_string());
    diagnostic
}

pub(super) fn import_load_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    candidates: &[PathBuf],
    error: impl std::fmt::Display,
    kind: ImportPathKind,
) -> Diagnostic {
    let searched = candidates
        .iter()
        .map(|path| format!("`{}`", path.display()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve import `{import_path}`; searched {searched}: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(match kind {
        ImportPathKind::Relative => {
            "relative imports are resolved from the importing file directory and automatically add `.nct`"
                .to_string()
        }
        ImportPathKind::NonRelative => {
            "non-relative imports are resolved inside the active Nocter home; `std/...` searches the active target overlay before common `std/`"
                .to_string()
        }
    });
    diagnostic
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportPathKind {
    Relative,
    NonRelative,
}

pub(super) fn nocter_home_import_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    error: impl std::fmt::Display,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve Nocter home while loading import `{import_path}`: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "set `NOCTER_HOME` to the active Nocter home, or run the `nocter` binary from inside its installed `.nocter/` directory"
            .to_string(),
    );
    diagnostic
}

pub(super) fn import_source_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    source_error: Diagnostic,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!(
            "failed to load import `{import_path}`: {}",
            source_error.message
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = source_error.help;
    diagnostic
}

pub(super) fn primitive_outside_nocter_home_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    target: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0414",
        "`primitive` declarations are allowed only inside the active Nocter home standard library",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "move the declaration under `std/` or `targets/{target}/std/` inside the active Nocter home"
    ));
    diagnostic
}
