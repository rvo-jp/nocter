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
            "relative imports are resolved from the importing file directory; a path may name a same-module `.nct` source file or a directory module's `index.nct`"
                .to_string()
        }
        ImportPathKind::PackageAbsolute => {
            "package-absolute imports name directory modules and resolve `index.nct` from the package root"
                .to_string()
        }
        ImportPathKind::NonRelative => {
            "non-relative imports name a declared dependency or standard-library directory module and resolve its `index.nct`"
                .to_string()
        }
    });
    diagnostic
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportPathKind {
    Relative,
    PackageAbsolute,
    NonRelative,
}

pub(super) fn package_absolute_import_without_package_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        "package-absolute imports require a package selected through `nocter.nct`",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("use a `./` or `../` import in explicit single-file mode".to_string());
    diagnostic
}

pub(super) fn undeclared_dependency_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    path: &str,
) -> Diagnostic {
    let name = path.split('/').next().unwrap_or(path);
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("import `{path}` names undeclared dependency `{name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "declare `{name}` in `#dependencies` or use `./`, `../`, or `/` for a package module"
    ));
    diagnostic
}

pub(super) fn package_import_escape_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    path: &str,
) -> Diagnostic {
    let mut diagnostic =
        Diagnostic::error("E0410", format!("import `{path}` escapes its package root"));
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("declare another package through `#dependencies` instead".to_string());
    diagnostic
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
            "ambiguous import `{import_path}`; both source file `{}` and child module directory `{}` exist",
            file.display(),
            directory.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "remove either the source file or the child module so the import has exactly one target"
            .to_string(),
    );
    diagnostic
}

pub(super) fn source_import_outside_module_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    path: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("source import `{path}` does not belong to the importing directory module"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "import a child module through its `index.nct` public surface, or move the source file under the current module without an intervening `index.nct`"
            .to_string(),
    );
    diagnostic
}

pub(super) fn invalid_source_import_declaration_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        "a same-module source import must be a private top-level `use ./path` declaration without an alias",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "source imports compose the current module and do not introduce a namespace; move the declaration to module scope and remove `pub`, imported names, and explicit aliases"
            .to_string(),
    );
    diagnostic
}

pub(super) fn public_declaration_outside_module_root_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0421",
        "`pub` declarations are allowed only in a module root source file (`index.nct`)",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "keep this declaration module-private or define the module's public surface in `index.nct`"
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
        "set `NOCTER_HOME` to the active Nocter home, or run `nocter` through a symlink to the installed `.nocter/nocter` binary"
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

pub(super) fn nocter_visibility_outside_nocter_home_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0420",
        "`pub(nocter)` declarations are allowed only inside the active Nocter home",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `pub` for public project API, omit `pub` for module-private API, or move the declaration inside the active Nocter home"
            .to_string(),
    );
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
