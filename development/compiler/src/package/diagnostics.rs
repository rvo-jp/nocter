use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};

pub(super) const PACKAGE_ERROR: &str = "E0800";

pub(super) fn package_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(PACKAGE_ERROR, message).with_primary_span_if_absent(sources, span)
}

pub(super) fn package_filesystem_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(PACKAGE_ERROR, message)
}
