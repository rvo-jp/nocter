use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;

use crate::DiscoveryError;
use crate::diagnostic::discovery_diagnostics;

/// One failed discovery plus the immutable source snapshot needed to present authored failures.
#[derive(Debug)]
pub struct DiscoveryFailure {
    error: Box<DiscoveryError>,
    sources: Box<SourceMap>,
    diagnostics: Box<[SourceDiagnostic]>,
}

impl DiscoveryFailure {
    pub(crate) fn before_source_snapshot(error: DiscoveryError) -> Self {
        Self {
            error: Box::new(error),
            sources: Box::new(SourceMap::new()),
            diagnostics: Box::new([]),
        }
    }

    pub(crate) fn from_snapshot(
        error: DiscoveryError,
        sources: SourceMap,
        syntax: &[SyntaxTree],
    ) -> Self {
        let (error, diagnostics) = match discovery_diagnostics(&error, syntax) {
            Ok(diagnostics) => (error, diagnostics),
            Err(projection_error) => (projection_error, Vec::new().into_boxed_slice()),
        };
        Self {
            error: Box::new(error),
            sources: Box::new(sources),
            diagnostics,
        }
    }

    #[must_use]
    pub const fn error(&self) -> &DiscoveryError {
        &self.error
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for DiscoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for DiscoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.error)
    }
}
