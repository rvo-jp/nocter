//! Immutable editor state shared by every LSP feature request.

mod build;
mod invalidation;
mod model;
mod store;

#[cfg(test)]
mod tests;

pub(in crate::driver::lsp) use invalidation::SnapshotChange;
pub(in crate::driver::lsp) use model::LspSnapshot;
pub(in crate::driver::lsp) use store::SnapshotStore;
