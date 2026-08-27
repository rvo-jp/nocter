use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_analysis::{AnalysisSnapshot, GenerationId};
use nocter_filesystem::SourceOverlay;

use crate::{WorkspaceAnalysisError, WorkspaceDiagnosticError};

/// One accepted source revision paired with the canonical document that triggered it.
#[derive(Clone, Debug)]
pub struct AcceptedDocumentRevision {
    path: PathBuf,
    source: nocter_analysis::WorkspaceSourceRevision,
}

impl AcceptedDocumentRevision {
    #[must_use]
    pub fn new(path: PathBuf, source: nocter_analysis::WorkspaceSourceRevision) -> Self {
        Self { path, source }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.source.generation()
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        self.source.source_overlay()
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (PathBuf, nocter_analysis::WorkspaceSourceRevision) {
        (self.path, self.source)
    }
}

/// The compiler input boundary selected for one document generation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisScope {
    Package(PathBuf),
    ToolchainStandard(PathBuf),
    SingleFile(PathBuf),
}

impl AnalysisScope {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Package(path) | Self::ToolchainStandard(path) | Self::SingleFile(path) => path,
        }
    }
}

/// The complete outcome retained for one scope in an accepted workspace generation.
#[derive(Debug)]
pub struct WorkspaceAnalysisGeneration {
    scope: Option<AnalysisScope>,
    invalidated: Box<[AnalysisScope]>,
    generation: GenerationId,
    state: WorkspaceAnalysisState,
}

impl WorkspaceAnalysisGeneration {
    pub(crate) fn new(
        scope: Option<AnalysisScope>,
        invalidated: Box<[AnalysisScope]>,
        generation: GenerationId,
        state: WorkspaceAnalysisState,
    ) -> Self {
        Self {
            scope,
            invalidated,
            generation,
            state,
        }
    }

    #[must_use]
    pub const fn scope(&self) -> Option<&AnalysisScope> {
        self.scope.as_ref()
    }

    #[must_use]
    pub const fn invalidated_scopes(&self) -> &[AnalysisScope] {
        &self.invalidated
    }

    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn snapshot(&self) -> Option<&AnalysisSnapshot> {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot),
            WorkspaceAnalysisState::PreparationFailed { .. }
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }

    #[must_use]
    pub const fn preparation_failure(&self) -> Option<&WorkspaceAnalysisError> {
        match &self.state {
            WorkspaceAnalysisState::PreparationFailed { error, .. } => Some(error),
            WorkspaceAnalysisState::Complete(_)
            | WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }

    /// Borrows every source-backed diagnostic owned by this generation.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when package preparation retained a diagnostic subject that is
    /// absent from its own reached source/syntax snapshot.
    pub fn diagnostics(
        &self,
    ) -> Result<&[nocter_diagnostics::SourceDiagnostic], WorkspaceDiagnosticError> {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => Ok(snapshot.diagnostics()),
            WorkspaceAnalysisState::PreparationFailed { diagnostics, .. } => {
                diagnostics.as_deref().map_err(|error| *error)
            }
            WorkspaceAnalysisState::InvalidationOnly { .. } => Ok(&[]),
        }
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => snapshot.source_overlay(),
            WorkspaceAnalysisState::PreparationFailed { source_overlay, .. }
            | WorkspaceAnalysisState::InvalidationOnly { source_overlay } => source_overlay,
        }
    }

    #[must_use]
    pub fn reached_sources(&self) -> Option<&nocter_source::SourceMap> {
        match &self.state {
            WorkspaceAnalysisState::Complete(snapshot) => Some(snapshot.sources()),
            WorkspaceAnalysisState::PreparationFailed { error, .. } => error
                .package_failure()
                .map(|failure| failure.reached().sources()),
            WorkspaceAnalysisState::InvalidationOnly { .. } => None,
        }
    }
}

/// One atomic workspace-analysis transition and every scope generation it refreshed.
#[derive(Debug)]
pub struct WorkspaceAnalysisBatch {
    primary: Arc<WorkspaceAnalysisGeneration>,
    related: Box<[Arc<WorkspaceAnalysisGeneration>]>,
}

impl WorkspaceAnalysisBatch {
    pub(crate) fn new(
        primary: Arc<WorkspaceAnalysisGeneration>,
        related: Box<[Arc<WorkspaceAnalysisGeneration>]>,
    ) -> Self {
        Self { primary, related }
    }

    #[must_use]
    pub fn primary(&self) -> &WorkspaceAnalysisGeneration {
        self.primary.as_ref()
    }

    pub fn publication_order(&self) -> impl Iterator<Item = &WorkspaceAnalysisGeneration> {
        self.related
            .iter()
            .map(Arc::as_ref)
            .chain(std::iter::once(self.primary.as_ref()))
    }

    #[must_use]
    pub fn into_generations(self) -> Box<[Arc<WorkspaceAnalysisGeneration>]> {
        std::iter::once(self.primary)
            .chain(self.related)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

#[derive(Debug)]
pub(crate) enum WorkspaceAnalysisState {
    Complete(Box<AnalysisSnapshot>),
    PreparationFailed {
        source_overlay: SourceOverlay,
        error: WorkspaceAnalysisError,
        diagnostics: Result<Box<[nocter_diagnostics::SourceDiagnostic]>, WorkspaceDiagnosticError>,
    },
    InvalidationOnly {
        source_overlay: SourceOverlay,
    },
}
