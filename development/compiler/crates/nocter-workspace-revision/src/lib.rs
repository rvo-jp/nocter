//! Protocol-independent ownership of accepted editor source revisions.

mod documents;
mod generation;

pub use documents::{
    DocumentChange, DocumentStateError, WorkspaceDocuments, WorkspaceRevisionSequence,
    WorkspaceSourceChange, WorkspaceSourceChangeKind, WorkspaceSourceRevision,
};
pub use generation::GenerationId;
