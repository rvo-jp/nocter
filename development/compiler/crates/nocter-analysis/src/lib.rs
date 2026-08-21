//! Immutable compiler analysis snapshots independent of any editor protocol.
//!
//! One [`AnalysisSnapshot`] owns one generation, its exact discovered source graph, and either the
//! compiler result or the failure reached by that same graph. Query layers cannot substitute a
//! previous successful program when the current generation fails.

use std::path::Path;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure};
use nocter_filesystem::{DocumentVersion, SourceOverlay};
use nocter_session::{CompileSessionError, CompiledTarget, compile_target};
use nocter_source::SourceMap;
use nocter_source_index::SourceIndex;
use nocter_syntax::SyntaxTree;

mod completion;
mod documents;
mod highlights;
mod navigation;
mod presentation;
mod rename;
mod semantic;
mod signature;

pub use completion::{SemanticCompletion, SemanticCompletionKind};
pub use documents::{
    AcceptedSourceGeneration, DocumentChange, DocumentStateError, WorkspaceDocuments,
};
pub use highlights::{SemanticHighlight, SemanticHighlightKind};
pub use navigation::SemanticLocation;
pub use presentation::SemanticPresentation;
pub use rename::{SemanticRenameEdit, SemanticRenameError, SemanticRenamePlan};
pub use semantic::{SemanticSelection, SemanticSubject};
pub use signature::{SemanticParameterLabel, SemanticSignatureHelp};

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
    SyntaxFailed(DiscoveredUnit),
    CompilationFailed {
        unit: DiscoveredUnit,
        error: CompileSessionError,
    },
    Complete {
        unit: DiscoveredUnit,
        target: Box<CompiledTarget>,
    },
}

/// One immutable generation shared by diagnostics and every semantic query.
#[derive(Debug)]
pub struct AnalysisSnapshot {
    generation: GenerationId,
    diagnostics: Box<[SourceDiagnostic]>,
    state: AnalysisState,
}

impl AnalysisSnapshot {
    fn discovered_unit(&self) -> Option<&DiscoveredUnit> {
        match &self.state {
            AnalysisState::Complete { unit, .. } => Some(unit),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::SyntaxFailed(_)
            | AnalysisState::CompilationFailed { .. } => None,
        }
    }

    /// Retains a failed discovery rather than falling back to an older source graph.
    #[must_use]
    pub fn from_discovery_failure(generation: GenerationId, failure: DiscoveryFailure) -> Self {
        Self {
            generation,
            diagnostics: failure.diagnostics().into(),
            state: AnalysisState::DiscoveryFailed(failure),
        }
    }

    /// Runs one discovered source graph through the production target-checking boundary.
    #[must_use]
    pub fn compile(generation: GenerationId, unit: DiscoveredUnit) -> Self {
        if unit.has_syntax_errors() {
            return Self {
                generation,
                diagnostics: unit.syntax_diagnostics(),
                state: AnalysisState::SyntaxFailed(unit),
            };
        }
        match compile_target(&unit) {
            Ok(target) => Self {
                generation,
                diagnostics: Box::new([]),
                state: AnalysisState::Complete {
                    unit,
                    target: Box::new(target),
                },
            },
            Err(error) => {
                let diagnostics = error
                    .source_diagnostic()
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Self {
                    generation,
                    diagnostics,
                    state: AnalysisState::CompilationFailed { unit, error },
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
            AnalysisState::CompilationFailed { .. } => AnalysisStatus::CompilationFailed,
            AnalysisState::Complete { .. } => AnalysisStatus::Complete,
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
            AnalysisState::SyntaxFailed(unit)
            | AnalysisState::CompilationFailed { unit, .. }
            | AnalysisState::Complete { unit, .. } => unit.source_overlay(),
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
            AnalysisState::SyntaxFailed(unit)
            | AnalysisState::CompilationFailed { unit, .. }
            | AnalysisState::Complete { unit, .. } => unit.sources(),
        }
    }

    #[must_use]
    pub fn syntax_trees(&self) -> &[SyntaxTree] {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => failure.syntax_trees(),
            AnalysisState::SyntaxFailed(unit)
            | AnalysisState::CompilationFailed { unit, .. }
            | AnalysisState::Complete { unit, .. } => unit.syntax_trees(),
        }
    }

    #[must_use]
    pub const fn source_index(&self) -> Option<&SourceIndex> {
        match &self.state {
            AnalysisState::Complete { target, .. } => Some(target.source_index()),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::SyntaxFailed(_)
            | AnalysisState::CompilationFailed { .. } => None,
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<&CompiledTarget> {
        match &self.state {
            AnalysisState::Complete { target, .. } => Some(target),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::SyntaxFailed(_)
            | AnalysisState::CompilationFailed { .. } => None,
        }
    }

    #[must_use]
    pub const fn discovery_failure(&self) -> Option<&DiscoveryFailure> {
        match &self.state {
            AnalysisState::DiscoveryFailed(failure) => Some(failure),
            AnalysisState::SyntaxFailed(_)
            | AnalysisState::CompilationFailed { .. }
            | AnalysisState::Complete { .. } => None,
        }
    }

    #[must_use]
    pub const fn compilation_failure(&self) -> Option<&CompileSessionError> {
        match &self.state {
            AnalysisState::CompilationFailed { error, .. } => Some(error),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::SyntaxFailed(_)
            | AnalysisState::Complete { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
