use std::path::Path;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure};
use nocter_filesystem::{DocumentVersion, SourceOverlay};
use nocter_session::{AnalyzedUnit, AnalyzedUnitStatus, SemanticEvidenceView};
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;
use nocter_workspace_revision::GenerationId;

use crate::query::AnalysisQuerySession;

/// The deepest immutable compiler boundary reached by one generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisStatus {
    DiscoveryFailed,
    SyntaxFailed,
    CompilationFailed,
    Complete,
}

#[derive(Debug)]
enum AnalysisState {
    DiscoveryFailed(DiscoveryFailure),
    Analyzed(Box<AnalyzedUnit>),
}

/// One immutable generation shared by diagnostics and every semantic query.
#[derive(Debug)]
pub struct AnalysisSnapshot {
    generation: GenerationId,
    diagnostics: Box<[SourceDiagnostic]>,
    state: AnalysisState,
    pub(crate) queries: AnalysisQuerySession,
}

impl AnalysisSnapshot {
    pub(crate) fn semantic_evidence(&self) -> Option<SemanticEvidenceView<'_>> {
        match &self.state {
            AnalysisState::Analyzed(analysis) => analysis.semantic_evidence(),
            AnalysisState::DiscoveryFailed(_) => None,
        }
    }

    pub(crate) fn current_unit(&self) -> Option<&DiscoveredUnit> {
        match &self.state {
            AnalysisState::Analyzed(analysis) => Some(analysis.unit()),
            AnalysisState::DiscoveryFailed(_) => None,
        }
    }

    /// Retains a failed discovery rather than falling back to an older source graph.
    #[must_use]
    pub fn from_discovery_failure(generation: GenerationId, failure: DiscoveryFailure) -> Self {
        let diagnostics = failure.diagnostics().into();
        let state = AnalysisState::DiscoveryFailed(failure);
        Self {
            generation,
            diagnostics,
            queries: AnalysisQuerySession::default(),
            state,
        }
    }

    /// Seals one session-owned source graph and its inseparable semantic outcome.
    #[must_use]
    pub fn from_analyzed_unit(generation: GenerationId, analysis: AnalyzedUnit) -> Self {
        Self {
            generation,
            diagnostics: analysis.diagnostics().into(),
            queries: AnalysisQuerySession::default(),
            state: AnalysisState::Analyzed(Box::new(analysis)),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn status(&self) -> AnalysisStatus {
        match self.state {
            AnalysisState::DiscoveryFailed(_) => AnalysisStatus::DiscoveryFailed,
            AnalysisState::Analyzed(ref analysis) => match analysis.status() {
                AnalyzedUnitStatus::SyntaxFailed => AnalysisStatus::SyntaxFailed,
                AnalyzedUnitStatus::CompilationFailed => AnalysisStatus::CompilationFailed,
                AnalyzedUnitStatus::Complete => AnalysisStatus::Complete,
            },
        }
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.source_overlay(),
            AnalysisState::Analyzed(analysis) => analysis.unit().source_overlay(),
        }
    }

    #[must_use]
    pub fn document_version(&self, canonical_path: &Path) -> Option<DocumentVersion> {
        self.source_overlay()
            .document(canonical_path)
            .map(nocter_filesystem::OpenDocument::version)
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.sources(),
            AnalysisState::Analyzed(analysis) => analysis.unit().sources(),
        }
    }

    #[must_use]
    pub(crate) fn syntax_trees(&self) -> &[SyntaxTree] {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.syntax_trees(),
            AnalysisState::Analyzed(analysis) => analysis.unit().syntax_trees(),
        }
    }
}
