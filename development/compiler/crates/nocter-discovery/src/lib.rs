//! Canonical filesystem-to-compile-input discovery for Nocter source graphs.
//!
//! Discovery is the only layer that probes package roots, source candidates, and directory-module
//! roots. Its immutable result retains exact physical ownership and one selected edge for every
//! active authored `use`; semantic consumers never reopen a path.

mod error;
mod graph;
mod request;
mod snapshot;
mod syntax;

pub use error::{DiscoveryError, ImportFailure, ToolchainDiscoveryError};
pub use graph::discover;
pub use request::{
    DiscoveryLayout, DiscoveryRequest, PrimitiveRoleLocator, StandardRoleLocator, ToolchainRequest,
};
pub use snapshot::{CompileInputError, DiscoveredModule, DiscoveredSource, DiscoveredUnit};

#[cfg(test)]
mod tests;
