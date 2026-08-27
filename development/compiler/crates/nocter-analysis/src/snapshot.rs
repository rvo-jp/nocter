use std::path::Path;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure};
use nocter_filesystem::{DocumentVersion, SourceOverlay};
use nocter_session::{
    CompiledTarget, SemanticEvidenceBundle, SemanticEvidenceView, analyze_incomplete_syntax,
    analyze_target,
};
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;

use crate::query::AnalysisQuerySession;

/// Monotonic identity assigned by the editor workspace that accepted a document state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(u64);

impl GenerationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

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
    SyntaxFailed(CurrentAnalysis),
    CompilationFailed(CurrentAnalysis),
    Complete(CurrentAnalysis),
}

#[derive(Debug)]
struct CurrentAnalysis {
    unit: Box<DiscoveredUnit>,
    semantic_evidence: CurrentSemanticEvidence,
}

#[derive(Debug)]
enum CurrentSemanticEvidence {
    Unavailable,
    Bundle(Box<SemanticEvidenceBundle>),
    Target(Box<CompiledTarget>),
}

impl CurrentSemanticEvidence {
    fn view(&self) -> Option<SemanticEvidenceView<'_>> {
        match self {
            Self::Unavailable => None,
            Self::Bundle(bundle) => Some(bundle.view()),
            Self::Target(target) => Some(target.semantic_evidence()),
        }
    }
}

impl CurrentAnalysis {
    fn recovered(unit: DiscoveredUnit, semantic: Option<SemanticEvidenceBundle>) -> Self {
        Self {
            unit: Box::new(unit),
            semantic_evidence: semantic.map_or(CurrentSemanticEvidence::Unavailable, |semantic| {
                CurrentSemanticEvidence::Bundle(Box::new(semantic))
            }),
        }
    }

    fn complete(unit: DiscoveredUnit, target: CompiledTarget) -> Self {
        Self {
            unit: Box::new(unit),
            semantic_evidence: CurrentSemanticEvidence::Target(Box::new(target)),
        }
    }
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
            AnalysisState::SyntaxFailed(analysis)
            | AnalysisState::CompilationFailed(analysis)
            | AnalysisState::Complete(analysis) => analysis.semantic_evidence.view(),
            AnalysisState::DiscoveryFailed(_) => None,
        }
    }

    pub(crate) fn current_unit(&self) -> Option<&DiscoveredUnit> {
        match &self.state {
            AnalysisState::SyntaxFailed(analysis)
            | AnalysisState::CompilationFailed(analysis)
            | AnalysisState::Complete(analysis) => Some(&analysis.unit),
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

    /// Runs one discovered source graph through the production target-checking boundary.
    #[must_use]
    pub fn compile(generation: GenerationId, unit: DiscoveredUnit) -> Self {
        if unit.has_syntax_errors() {
            let (semantic, semantic_diagnostics) = analyze_incomplete_syntax(&unit).map_or(
                (
                    None,
                    Box::<[nocter_diagnostics::SourceDiagnostic]>::default(),
                ),
                nocter_session::IncompleteSyntaxAnalysis::into_analysis_parts,
            );
            let mut diagnostics = unit.syntax_diagnostics().into_vec();
            extend_unique_diagnostics(&mut diagnostics, &semantic_diagnostics);
            let state = AnalysisState::SyntaxFailed(CurrentAnalysis::recovered(unit, semantic));
            return Self {
                generation,
                diagnostics: diagnostics.into_boxed_slice(),
                queries: AnalysisQuerySession::default(),
                state,
            };
        }
        match analyze_target(&unit) {
            Ok(target) => {
                let state = AnalysisState::Complete(CurrentAnalysis::complete(unit, target));
                Self {
                    generation,
                    diagnostics: Box::new([]),
                    queries: AnalysisQuerySession::default(),
                    state,
                }
            }
            Err(failure) => {
                let (semantic, diagnostics) = (*failure).into_analysis_parts();
                let state =
                    AnalysisState::CompilationFailed(CurrentAnalysis::recovered(unit, semantic));
                Self {
                    generation,
                    diagnostics,
                    queries: AnalysisQuerySession::default(),
                    state,
                }
            }
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
            AnalysisState::SyntaxFailed(_) => AnalysisStatus::SyntaxFailed,
            AnalysisState::CompilationFailed(_) => AnalysisStatus::CompilationFailed,
            AnalysisState::Complete(_) => AnalysisStatus::Complete,
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
            AnalysisState::SyntaxFailed(analysis)
            | AnalysisState::CompilationFailed(analysis)
            | AnalysisState::Complete(analysis) => analysis.unit.source_overlay(),
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
            AnalysisState::SyntaxFailed(analysis)
            | AnalysisState::CompilationFailed(analysis)
            | AnalysisState::Complete(analysis) => analysis.unit.sources(),
        }
    }

    #[must_use]
    pub(crate) fn syntax_trees(&self) -> &[SyntaxTree] {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.syntax_trees(),
            AnalysisState::SyntaxFailed(analysis)
            | AnalysisState::CompilationFailed(analysis)
            | AnalysisState::Complete(analysis) => analysis.unit.syntax_trees(),
        }
    }
}

pub(crate) fn extend_unique_diagnostics(
    diagnostics: &mut Vec<SourceDiagnostic>,
    candidates: &[SourceDiagnostic],
) {
    for diagnostic in candidates {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
}
