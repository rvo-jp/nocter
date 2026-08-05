use super::diagnostics::package_diagnostic;
use crate::ast::PackageHeader;
use crate::diagnostics::Diagnostic;
use crate::source::{SourceId, SourceMap};
use std::path::Path;

pub(crate) fn validate_package_header_location(
    sources: &SourceMap,
    source: SourceId,
    header: &PackageHeader,
    source_root: Option<&Path>,
) -> Vec<Diagnostic> {
    let Some(first) = header.directives.first() else {
        return Vec::new();
    };
    let Some(source_path) = sources.get(source).and_then(|file| file.absolute_path()) else {
        return vec![package_diagnostic(
            sources,
            first.span,
            "package directives are only allowed in a package root `index.nct`",
        )];
    };
    let Some(root) = source_root else {
        return vec![package_diagnostic(
            sources,
            first.span,
            "package directives require package mode and are only allowed in `index.nct`",
        )];
    };
    let expected = root.join("index.nct");
    let expected = std::fs::canonicalize(&expected).unwrap_or(expected);
    if source_path == expected.as_path() {
        Vec::new()
    } else {
        vec![package_diagnostic(
            sources,
            first.span,
            format!(
                "package directives are only allowed in `{}`",
                expected.display()
            ),
        )]
    }
}
