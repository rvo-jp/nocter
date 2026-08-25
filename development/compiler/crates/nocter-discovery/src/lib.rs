//! Canonical filesystem-to-compile-input discovery for Nocter source graphs.
//!
//! Discovery is the only layer that probes package roots, source candidates, and directory-module
//! roots. Its immutable result retains exact physical ownership and one selected edge for every
//! active authored `see` and `use`; semantic consumers never reopen a path.

mod diagnostic;
mod error;
mod failure;
mod graph;
mod module_catalog;
mod request;
mod snapshot;
mod source_visibility;
mod syntax;

pub use error::{DiscoveryError, SourceVisibilityFailure, ToolchainDiscoveryError, UseFailure};
pub use failure::DiscoveryFailure;
pub use graph::discover;
pub use module_catalog::module_for_source;
pub use request::{DiscoveryLayout, DiscoveryRequest};
pub use snapshot::{
    CompileInputError, DiscoveredModule, DiscoveredModuleDependency, DiscoveredSource,
    DiscoveredUnit,
};

#[cfg(test)]
mod tests;
