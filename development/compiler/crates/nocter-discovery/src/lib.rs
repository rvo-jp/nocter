//! Canonical filesystem-to-compile-input discovery for Nocter source graphs.
//!
//! Discovery extends the package layer's revision-local root catalog while it selects source
//! candidates and directory-module roots. Its immutable result retains exact physical ownership
//! and one selected edge for every active authored `see` and `use`; semantic consumers never
//! reopen a path.

mod current_source;
mod diagnostic;
mod error;
mod failure;
mod graph;
mod module_catalog;
mod request;
mod semantic_topology;
mod snapshot;
mod source_domain;
mod source_visibility;
mod syntax;

pub use current_source::{CurrentSourceSurface, CurrentSourceSurfaceError};
pub use error::{DiscoveryError, SourceVisibilityFailure, ToolchainDiscoveryError, UseFailure};
pub use failure::DiscoveryFailure;
pub use graph::{discover, discover_with_source_syntax};
pub use module_catalog::module_for_source;
pub use request::{DiscoveryLayout, DiscoveryRequest};
pub use semantic_topology::{SemanticTopologyError, SemanticTopologySurface};
pub use snapshot::{
    CompileInputError, DiscoveredModule, DiscoveredModuleDependency, DiscoveredSource,
    DiscoveredUnit,
};
pub use source_domain::SourceDomainError;

#[cfg(test)]
mod tests;
