//! Package declaration and exact dependency-graph authority.
//!
//! This crate owns the data interpretation of `nocter.nct`. Source discovery and semantic
//! lowering consume its structured facts and exact syntax origins; they must not decode package
//! directive fields independently.

mod declaration;
mod graph;
mod id;
mod lock;
mod lock_overlay;
mod resolution;
mod store_overlay;

pub use declaration::{
    AuthoredString, DependencyDeclaration, DependencyLock, DependencySource, PackageDeclaration,
    PackageDeclarationError, PackageDeclarationRule, PackageTargetDeclaration,
    decode_package_declaration,
};
pub use graph::{
    PackageGraphError, ResolvedPackageGraph, ResolvedPackageSnapshot, ResolvedPackageSpec,
};
pub use id::{PackageId, PackageIdError};
pub use lock::{ExactDependencyLock, ExactDependencyLockError, ExactDependencyLockKind};
pub use lock_overlay::{PackageLockOverlay, PackageLockOverlayError};
pub use resolution::{
    PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    ResolvedPackageSelection, StandardPackage, resolve_package_graph, resolve_package_selection,
    resolve_standard_package,
};
pub use store_overlay::{PackageStoreOverlay, PackageStoreOverlayError};
