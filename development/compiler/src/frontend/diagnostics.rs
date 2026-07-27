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
            "relative imports are resolved from the importing file directory; module paths try `.nct` and then `index.nct`"
                .to_string()
        }
        ImportPathKind::Absolute => {
            "absolute imports are resolved from the filesystem root; module paths try `.nct` and then `index.nct`"
                .to_string()
        }
        ImportPathKind::NonRelative => {
            "non-relative imports are resolved from the source root first and the active Nocter home second; module paths try `.nct` and then `index.nct`"
                .to_string()
        }
    });
    diagnostic
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportPathKind {
    Relative,
    Absolute,
    NonRelative,
}

pub(super) fn ambiguous_import_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    file: &std::path::Path,
    directory: &std::path::Path,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!(
            "ambiguous import `{import_path}`; both module file `{}` and module directory `{}` exist",
            file.display(),
            directory.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "remove either the module file or the directory so the import has exactly one target"
            .to_string(),
    );
    diagnostic
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
        "move the declaration under `std/` inside the active Nocter home for target `{target}`"
    ));
    diagnostic
}

pub(super) fn primitive_registry_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    message: impl Into<String>,
    help: Option<String>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0415", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = help;
    diagnostic
}
