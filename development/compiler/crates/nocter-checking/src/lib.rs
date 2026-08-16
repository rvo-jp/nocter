//! One-way construction of the syntax-independent checked program.
//!
//! This crate is the only Phase 3 boundary allowed to inspect body syntax. It consumes the
//! declaration-lowering result and extends the separate source projection while constructing
//! checked semantic identities. Target validation and later lowering cannot depend on this crate.

mod body_sources;
mod conformance;
mod names;

pub use body_sources::{BodySource, BodySourceCatalog, BodySourceError, catalog_body_sources};
pub use conformance::{
    CheckedConformance, CheckedPredicate, CheckedRequirement, ConformanceBuildError,
    ConformanceInternalError, ConformanceMethod, ConformanceRule, ConformanceTable,
    MethodSelection, SubstitutionError, build_conformance_table,
};
pub use names::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameResolution,
    NameResolutionError, NameRule, NameTarget, ResolvedBodyNames, ResolvedNameUse,
    resolve_body_names,
};
