//! Package declaration and exact dependency-graph authority.
//!
//! This crate owns the data interpretation of `index.nct`. Source discovery and semantic
//! lowering consume its structured facts and exact syntax origins; they must not decode package
//! directive fields independently.

mod declaration;
mod graph;
mod id;
mod lock;
mod lock_overlay;
mod resolution;
mod root_probe;
mod selection_source;
mod store_overlay;

pub use declaration::{
    AuthoredString, DependencyDeclaration, DependencyExactSelection, DependencySource,
    PackageDeclaration, PackageDeclarationError, PackageDeclarationRule, PackageTargetDeclaration,
    decode_package_declaration,
};
pub use graph::{
    PackageGraphError, PackageSourceSnapshot, ResolvedPackageGraph, ResolvedPackageSnapshot,
    ResolvedPackageSpec,
};
pub use id::{PackageId, PackageIdError};
pub use lock::{ExactDependencyLock, ExactDependencyLockError, ExactDependencyLockKind};
pub use lock_overlay::{PackageLockOverlay, PackageLockOverlayError};
pub use resolution::{
    PackageResolutionError, PackageResolutionFailure, PackageResolutionPolicy,
    PackageResolutionRequest, ResolvedPackageSelection, StandardPackage,
    resolve_package_selection_with_root_catalog, resolve_standard_package_with_root_catalog,
};
pub use root_probe::{PackageRootCatalog, PackageRootCatalogBuilder, PackageRootProbeError};
pub use selection_source::{PackageExactSelectionSourceError, PackageExactSelectionSourceUpdate};
pub use store_overlay::{PackageStoreOverlay, PackageStoreOverlayError};
