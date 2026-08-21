//! Package declaration and exact dependency-graph authority.
//!
//! This crate owns the data interpretation of `nocter.nct`. Source discovery and semantic
//! lowering consume its structured facts and exact syntax origins; they must not decode package
//! directive fields independently.

mod declaration;
mod graph;
mod id;
mod resolution;

pub use declaration::{
    AuthoredString, DependencyDeclaration, DependencyLock, DependencySource, PackageDeclaration,
    PackageDeclarationError, PackageDeclarationRule, PackageTargetDeclaration,
    decode_package_declaration,
};
pub use graph::{
    PackageGraphError, ResolvedPackageGraph, ResolvedPackageSnapshot, ResolvedPackageSpec,
};
pub use id::{PackageId, PackageIdError};
pub use resolution::{
    PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    ResolvedPackageSelection, StandardPackage, resolve_package_graph, resolve_package_selection,
    resolve_standard_package,
};
