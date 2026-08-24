//! Immutable compiler analysis snapshots independent of any editor protocol.
//!
//! One [`AnalysisSnapshot`] owns one generation, its exact discovered source graph, and either the
//! compiler result or the failure reached by that same graph. Query layers cannot substitute a
//! previous successful program when the current generation fails.

use std::path::Path;

use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::{DiscoveredUnit, DiscoveryFailure};
use nocter_filesystem::{DocumentVersion, SourceOverlay};
use nocter_session::{
    CompileSessionError, CompiledTarget, SemanticAnalysis, analyze_incomplete_syntax,
    analyze_target,
};
use nocter_source::SourceMap;
use nocter_source_index::SourceIndex;
use nocter_syntax::SyntaxTree;

mod callable_source;
mod code_actions;
mod completion;
mod documents;
mod highlights;
mod inlay_hints;
mod navigation;
mod presentation;
mod rename;
mod semantic;
mod signature;
mod source_context;
mod source_edits;
mod source_selection;

pub use code_actions::{
    ConformanceActionError, OutcomeActionError, SemanticCodeAction, SemanticCodeActionError,
};
pub use completion::{
    SemanticCompletion, SemanticCompletionEdit, SemanticCompletionError, SemanticCompletionKind,
};
pub use documents::{
    AcceptedSourceGeneration, DocumentChange, DocumentStateError, WorkspaceDocuments,
};
pub use highlights::{SemanticHighlight, SemanticHighlightKind};
pub use inlay_hints::{SemanticInlayHint, SemanticInlayHintError, SemanticInlayHintKind};
pub use navigation::SemanticLocation;
pub use presentation::{PresentationError, SemanticPresentation};
pub use rename::{SemanticRenameEdit, SemanticRenameError, SemanticRenamePlan};
pub use semantic::{SemanticQueryError, SemanticSelection, SemanticSubject};
pub use signature::{SemanticParameterLabel, SemanticSignatureHelp};
pub use source_context::SourceContextError;
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
    authority: CurrentSemanticAuthority,
}

#[derive(Debug)]
enum CurrentAnalysisFailure {
    Syntax,
    Compilation(CompileSessionError),
}

#[derive(Debug)]
enum CurrentSemanticAuthority {
    None,
    Semantic(Box<SemanticAnalysis>),
    Target(Box<CompiledTarget>),
}

impl CurrentAnalysis {
    fn syntax(unit: DiscoveredUnit, semantic: Option<SemanticAnalysis>) -> Self {
        Self {
            unit: Box::new(unit),
            failure: Some(CurrentAnalysisFailure::Syntax),
            authority: semantic.map_or(CurrentSemanticAuthority::None, |semantic| {
                CurrentSemanticAuthority::Semantic(Box::new(semantic))
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
            authority: semantic.map_or(CurrentSemanticAuthority::None, |semantic| {
                CurrentSemanticAuthority::Semantic(Box::new(semantic))
            }),
        }
    }

    fn complete(unit: DiscoveredUnit, target: CompiledTarget) -> Self {
        Self {
            unit: Box::new(unit),
            failure: None,
            authority: CurrentSemanticAuthority::Target(Box::new(target)),
        }
    }
}

/// One immutable generation shared by diagnostics and every semantic query.
#[derive(Debug)]
pub struct AnalysisSnapshot {
    generation: GenerationId,
    diagnostics: Box<[SourceDiagnostic]>,
    state: AnalysisState,
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
            let semantic = analyze_incomplete_syntax(&unit);
            return Self {
                generation,
                diagnostics: unit.syntax_diagnostics(),
                state: AnalysisState::Current(CurrentAnalysis::syntax(unit, semantic)),
            };
        }
        match analyze_target(&unit) {
            Ok(target) => Self {
                generation,
                diagnostics: Box::new([]),
                state: AnalysisState::Current(CurrentAnalysis::complete(unit, target)),
            },
            Err(failure) => {
                let (error, semantic) = (*failure).into_parts();
                let diagnostics = error
                    .source_diagnostic()
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Self {
                    generation,
                    diagnostics,
                    state: AnalysisState::Current(CurrentAnalysis::compilation(
                        unit, error, semantic,
                    )),
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
                failure: Some(CurrentAnalysisFailure::Syntax),
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

    /// Reports whether source semantics completed through type and body checking.
    ///
    /// Target construction may still have failed for an independent toolchain or ABI reason.
    /// Semantic mutation validation uses this capability instead of conflating it with executable
    /// target availability.
    #[must_use]
    pub fn has_checked_semantics(&self) -> bool {
        self.semantic_authority()
            .and_then(|authority| authority.checked())
            .is_some()
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
    pub const fn source_index(&self) -> Option<&SourceIndex> {
        match &self.state {
            AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::Target(target),
                ..
            }) => Some(target.source_index()),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::None | CurrentSemanticAuthority::Semantic(_),
                ..
            }) => None,
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<&CompiledTarget> {
        match &self.state {
            AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::Target(target),
                ..
            }) => Some(target),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::None | CurrentSemanticAuthority::Semantic(_),
                ..
            }) => None,
        }
    }

    pub(crate) fn retained_semantic(&self) -> Option<&SemanticAnalysis> {
        match &self.state {
            AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::Semantic(semantic),
                ..
            }) => Some(semantic),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::None | CurrentSemanticAuthority::Target(_),
                ..
            }) => None,
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
            AnalysisState::Current(CurrentAnalysis {
                failure: Some(CurrentAnalysisFailure::Compilation(error)),
                ..
            }) => Some(error),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                failure: None | Some(CurrentAnalysisFailure::Syntax),
                ..
            }) => None,
        }
    }
}

#[cfg(test)]
mod tests;
