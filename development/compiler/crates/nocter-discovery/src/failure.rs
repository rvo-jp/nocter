use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_filesystem::SourceOverlay;
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;

use crate::DiscoveryError;
use crate::diagnostic::discovery_diagnostics;

/// One failed discovery plus the immutable source snapshot needed to present authored failures.
#[derive(Debug)]
pub struct DiscoveryFailure {
    error: Box<DiscoveryError>,
    source_overlay: SourceOverlay,
    sources: Box<SourceMap>,
    syntax: Box<[SyntaxTree]>,
    diagnostics: Box<[SourceDiagnostic]>,
}

impl DiscoveryFailure {
    pub(crate) fn before_source_snapshot(
        error: DiscoveryError,
        source_overlay: SourceOverlay,
    ) -> Self {
        Self {
            error: Box::new(error),
            source_overlay,
            sources: Box::new(SourceMap::new()),
            syntax: Box::new([]),
            diagnostics: Box::new([]),
        }
    }

    pub(crate) fn from_snapshot(
        error: DiscoveryError,
        source_overlay: SourceOverlay,
        sources: SourceMap,
        syntax: Vec<SyntaxTree>,
    ) -> Self {
        let (error, diagnostics) = match discovery_diagnostics(&error, &syntax) {
            Ok(diagnostics) => (error, diagnostics),
            Err(projection_error) => (projection_error, Vec::new().into_boxed_slice()),
        };
        Self {
            error: Box::new(error),
            source_overlay,
            sources: Box::new(sources),
            syntax: syntax.into_boxed_slice(),
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
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.source_overlay
    }

    #[must_use]
    pub const fn syntax_trees(&self) -> &[SyntaxTree] {
        &self.syntax
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
