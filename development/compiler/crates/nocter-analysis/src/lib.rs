//! Immutable compiler analysis snapshots independent of any editor protocol.
//!
//! One [`AnalysisSnapshot`] owns one generation, its exact discovered source graph, its diagnostic
//! outcome, and its deepest completed semantic authority as independent facts. Query layers cannot
//! substitute a previous successful program when the current generation fails.

mod documents;
mod query;
mod snapshot;
mod source_edits;

pub use documents::{
    DocumentChange, DocumentStateError, WorkspaceDocuments, WorkspaceRevisionSequence,
    WorkspaceSourceChange, WorkspaceSourceChangeKind, WorkspaceSourceRevision,
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
pub use snapshot::{AnalysisSnapshot, AnalysisStatus, GenerationId};
pub use source_edits::{
    SemanticMutationBuildError, SemanticMutationCandidate, SemanticSourceEdit,
    SemanticSourceEditGroup, ValidatedSemanticMutation,
};

#[cfg(test)]
pub(crate) mod tests;
