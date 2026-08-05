//! Source-native package loading and executable-target selection.

mod dependency;
mod diagnostics;
mod fetch;
mod graph;
mod loader;
mod lockfile;
mod model;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use dependency::{DependencyDeclaration, DependencyLock, DependencySource, LockedDependency};
pub use graph::{PackageGraph, PackageGraphLoad, PackageGraphOptions, load_package_graph};
pub use loader::{PackageLoad, load_package};
pub use model::{ExecutableId, ExecutableTarget, ModuleId, PackageId, SourcePackage};
pub(crate) use validation::resolve_package_module;
