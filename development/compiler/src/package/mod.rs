//! Source-native package loading and executable-target selection.

mod dependency;
mod diagnostics;
mod fetch;
mod graph;
mod loader;
mod lockfile;
mod model;
mod modules;
mod store;
mod targets;
mod validation;

#[cfg(test)]
mod tests;

pub use dependency::{DependencyDeclaration, DependencyLock, DependencySource, LockedDependency};
pub use graph::{PackageGraph, PackageGraphLoad, PackageGraphOptions, load_package_graph};
pub use loader::{PackageLoad, load_package};
pub use model::{
    ExecutableId, ExecutableTarget, ModuleId, ModuleKey, NormalizedModulePath, PackageId,
    ResolvedModule, SourcePackage,
};
pub(crate) use modules::resolve_explicit_module_path;
pub(crate) use targets::executable_entry_at_offset;
