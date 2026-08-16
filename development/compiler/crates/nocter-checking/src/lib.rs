//! One-way construction of the syntax-independent checked program.
//!
//! This crate is the only Phase 3 boundary allowed to inspect body syntax. It consumes the
//! declaration-lowering result and extends the separate source projection while constructing
//! checked semantic identities. Target validation and later lowering cannot depend on this crate.

mod body_sources;

pub use body_sources::{BodySource, BodySourceCatalog, BodySourceError, catalog_body_sources};
