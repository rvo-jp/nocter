//! Filesystem and compiler-analysis composition for the Nocter language server.
//!
//! Protocol decoding remains in `nocter-lsp`; this crate is the first layer allowed to resolve a
//! document URI through the filesystem and mutate accepted workspace document state.

mod analysis;
mod diagnostics;
mod documents;
mod paths;
mod run;
mod server;
mod workspace;

pub use analysis::{
    AnalysisScope, WorkspaceAnalyses, WorkspaceAnalysisError, WorkspaceAnalysisGeneration,
};
pub use diagnostics::{DiagnosticPublicationError, DiagnosticPublisher};
pub use documents::{
    AcceptedDocumentGeneration, DocumentWorkspace, DocumentWorkspaceChange, DocumentWorkspaceError,
};
pub use paths::{DocumentPathError, DocumentPathResolver};
pub use run::{LanguageServerExit, LanguageServerRunError, run_language_server};
pub use server::{ClientResponseError, LanguageServer, ServerIssue, ServerStep};
pub use workspace::{
    LanguageServerEnvironment, LanguageServerToolchain, WorkspaceConfiguration,
    WorkspaceConfigurationError,
};
