//! Immutable package-wide semantic occurrence index.
//!
//! Individual editor analyses own independent `SourceMap` values, so their
//! numeric `SourceId` values cannot be compared. This module translates the
//! resolver-owned semantic identities into source-backed identities before
//! joining occurrences from multiple compile units.

mod build;
mod model;

pub(crate) use build::{PackageSemanticIndexBuilder, stable_semantic_identity_at};
pub(crate) use model::{PackageSemanticIndex, RenamePlan};
