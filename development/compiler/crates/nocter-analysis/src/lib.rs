//! Immutable compiler analysis snapshots independent of any editor protocol.
//!
//! One [`AnalysisSnapshot`] owns one generation, its exact discovered source graph, its diagnostic
//! outcome, and its deepest completed semantic authority as independent facts. Query layers cannot
//! substitute a previous successful program when the current generation fails.

use std::path::Path;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure};
use nocter_filesystem::{DocumentVersion, SourceOverlay};
use nocter_session::{
    CompileSessionError, CompiledTarget, SemanticAnalysis, analyze_incomplete_syntax,
    analyze_target,
};
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;

mod documents;
mod query;
mod source_edits;

pub use documents::{
    AcceptedSourceGeneration, DocumentChange, DocumentStateError, WorkspaceDocuments,
};
pub use query::{
    EvidenceIntegrityError, InterfaceImplementationActionError, OutcomeActionError,
    PresentationError, SemanticBodyGap, SemanticCodeAction, SemanticCodeActionError,
    SemanticCompletion, SemanticCompletionEdit, SemanticCompletionError, SemanticCompletionKind,
    SemanticCoverage, SemanticHighlight, SemanticHighlightKind, SemanticInlayHint,
    SemanticInlayHintError, SemanticInlayHintKind, SemanticLocation, SemanticParameterLabel,
    SemanticPresentation, SemanticQueryError, SemanticQuerySet, SemanticRenameEdit,
    SemanticRenameError, SemanticRenamePlan, SemanticSelection, SemanticSetUnavailability,
    SemanticSignatureError, SemanticSignatureHelp, SemanticSubject, SourceContextError,
    TypedBodyUnavailability,
};
pub use source_edits::SemanticSourceEdit;

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
    Current(CurrentAnalysis),
}

#[derive(Debug)]
struct CurrentAnalysis {
    unit: Box<DiscoveredUnit>,
    failure: Option<CurrentAnalysisFailure>,
    semantic_evidence: CurrentSemanticEvidence,
}

#[derive(Debug)]
enum CurrentAnalysisFailure {
    Syntax(Option<CompileSessionError>),
    Compilation(CompileSessionError),
}

#[derive(Debug)]
enum CurrentSemanticEvidence {
    Unavailable,
    Analysis(Box<SemanticAnalysis>),
    Target(Box<CompiledTarget>),
}

impl CurrentAnalysis {
    fn syntax(
        unit: DiscoveredUnit,
        failure: Option<CompileSessionError>,
        semantic: Option<SemanticAnalysis>,
    ) -> Self {
        Self {
            unit: Box::new(unit),
            failure: Some(CurrentAnalysisFailure::Syntax(failure)),
            semantic_evidence: semantic.map_or(CurrentSemanticEvidence::Unavailable, |semantic| {
                CurrentSemanticEvidence::Analysis(Box::new(semantic))
            }),
        }
    }

    fn compilation(
        unit: DiscoveredUnit,
        error: CompileSessionError,
        semantic: Option<SemanticAnalysis>,
    ) -> Self {
        Self {
            unit: Box::new(unit),
            failure: Some(CurrentAnalysisFailure::Compilation(error)),
            semantic_evidence: semantic.map_or(CurrentSemanticEvidence::Unavailable, |semantic| {
                CurrentSemanticEvidence::Analysis(Box::new(semantic))
            }),
        }
    }

    fn complete(unit: DiscoveredUnit, target: CompiledTarget) -> Self {
        Self {
            unit: Box::new(unit),
            failure: None,
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
    queries: query::AnalysisQuerySession,
}

impl AnalysisSnapshot {
    fn current_unit(&self) -> Option<&DiscoveredUnit> {
        match &self.state {
            AnalysisState::Current(analysis) => Some(&analysis.unit),
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
            queries: query::AnalysisQuerySession::default(),
            state,
        }
    }

    /// Runs one discovered source graph through the production target-checking boundary.
    #[must_use]
    pub fn compile(generation: GenerationId, unit: DiscoveredUnit) -> Self {
        if unit.has_syntax_errors() {
            let (failure, semantic, semantic_diagnostics) = analyze_incomplete_syntax(&unit)
                .map_or(
                    (
                        None,
                        None,
                        Box::<[nocter_diagnostics::SourceDiagnostic]>::default(),
                    ),
                    nocter_session::IncompleteSyntaxAnalysis::into_parts,
                );
            let mut diagnostics = unit.syntax_diagnostics().into_vec();
            extend_unique_diagnostics(&mut diagnostics, &semantic_diagnostics);
            let state = AnalysisState::Current(CurrentAnalysis::syntax(unit, failure, semantic));
            return Self {
                generation,
                diagnostics: diagnostics.into_boxed_slice(),
                queries: query::AnalysisQuerySession::default(),
                state,
            };
        }
        match analyze_target(&unit) {
            Ok(target) => {
                let state = AnalysisState::Current(CurrentAnalysis::complete(unit, target));
                Self {
                    generation,
                    diagnostics: Box::new([]),
                    queries: query::AnalysisQuerySession::default(),
                    state,
                }
            }
            Err(failure) => {
                let (error, semantic, diagnostics) = (*failure).into_parts();
                let state =
                    AnalysisState::Current(CurrentAnalysis::compilation(unit, error, semantic));
                Self {
                    generation,
                    diagnostics,
                    queries: query::AnalysisQuerySession::default(),
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
            AnalysisState::Current(CurrentAnalysis {
                failure: Some(CurrentAnalysisFailure::Syntax(_)),
                ..
            }) => AnalysisStatus::SyntaxFailed,
            AnalysisState::Current(CurrentAnalysis {
                failure: Some(CurrentAnalysisFailure::Compilation(_)),
                ..
            }) => AnalysisStatus::CompilationFailed,
            AnalysisState::Current(CurrentAnalysis { failure: None, .. }) => {
                AnalysisStatus::Complete
            }
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
            AnalysisState::Current(analysis) => analysis.unit.source_overlay(),
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
            AnalysisState::Current(analysis) => analysis.unit.sources(),
        }
    }

    #[must_use]
    pub fn syntax_trees(&self) -> &[SyntaxTree] {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.syntax_trees(),
            AnalysisState::Current(analysis) => analysis.unit.syntax_trees(),
        }
    }

    #[must_use]
    pub const fn discovery_failure(&self) -> Option<&DiscoveryFailure> {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => Some(failure),
            AnalysisState::Current(_) => None,
        }
    }

    #[must_use]
    pub const fn compilation_failure(&self) -> Option<&CompileSessionError> {
        match &self.state {
            AnalysisState::Current(
                CurrentAnalysis {
                    failure: Some(CurrentAnalysisFailure::Syntax(Some(error))),
                    ..
                }
                | CurrentAnalysis {
                    failure: Some(CurrentAnalysisFailure::Compilation(error)),
                    ..
                },
            ) => Some(error),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                failure: None | Some(CurrentAnalysisFailure::Syntax(None)),
                ..
            }) => None,
        }
    }
}

fn extend_unique_diagnostics(
    diagnostics: &mut Vec<SourceDiagnostic>,
    candidates: &[SourceDiagnostic],
) {
    for diagnostic in candidates {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
