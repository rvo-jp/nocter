//! Filesystem and compiler-analysis composition for the Nocter language server.
//!
//! Protocol decoding remains in `nocter-lsp`; this crate is the first layer allowed to resolve a
//! document URI through the filesystem and mutate accepted workspace document state.

mod documents;
mod paths;

pub use documents::{DocumentWorkspace, DocumentWorkspaceError};
pub use paths::{DocumentPathError, DocumentPathResolver};
