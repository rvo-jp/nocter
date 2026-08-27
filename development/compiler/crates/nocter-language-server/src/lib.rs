//! Filesystem and compiler-analysis composition for the Nocter language server.
//!
//! Protocol decoding remains in `nocter-lsp`; this crate is the first layer allowed to resolve a
//! document URI through the filesystem and mutate accepted workspace document state.

mod analysis;
mod code_actions;
mod completion;
mod diagnostics;
mod documents;
mod hover;
mod inlay_hints;
mod navigation;
mod paths;
mod rename;
mod run;
mod semantic_document;
mod semantic_tokens;
mod server;
mod signature;
mod workspace;
mod workspace_edits;

pub use analysis::{
    AnalysisScope, WorkspaceAnalyses, WorkspaceAnalysisBatch, WorkspaceAnalysisError,
    WorkspaceAnalysisGeneration,
};
pub use code_actions::CodeActionQueryError;
pub use completion::CompletionQueryError;
pub use diagnostics::{DiagnosticPublicationError, DiagnosticPublisher};
pub use documents::{
    AcceptedDocumentRevision, DocumentWorkspace, DocumentWorkspaceChange, DocumentWorkspaceError,
};
pub use hover::HoverQueryError;
pub use inlay_hints::InlayHintQueryError;
pub use navigation::NavigationQueryError;
pub use paths::{DocumentPathError, DocumentPathResolver};
pub use rename::RenameQueryError;
pub use run::{LanguageServerExit, LanguageServerRunError, run_language_server};
pub use semantic_document::SemanticDocumentError;
pub use semantic_tokens::SemanticTokensQueryError;
pub use server::{ClientResponseError, LanguageServer, ServerIssue, ServerStep};
pub use signature::SignatureQueryError;
pub use workspace::{
    LanguageServerEnvironment, LanguageServerToolchain, WorkspaceConfiguration,
    WorkspaceConfigurationError,
};
