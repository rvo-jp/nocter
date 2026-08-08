//! Source-native package loading and executable-target selection.

mod dependency;
mod diagnostics;
mod fetch;
mod graph;
mod loader;
mod lockfile;
mod model;
mod modules;
mod overlay;
mod store;
mod targets;
mod test_targets;
mod validation;

#[cfg(test)]
mod tests;

pub use dependency::{DependencyDeclaration, DependencyLock, DependencySource, LockedDependency};
pub(crate) use graph::load_locked_offline_package_graph_with_overlay;
pub use graph::{
    PackageGraph, PackageGraphLoad, PackageGraphOptions, inspect_package_graph, load_package_graph,
};
pub use loader::{PackageLoad, load_package};
pub use model::{
    ExecutableId, ExecutableTarget, ModuleId, ModuleKey, NormalizedModulePath, PackageId,
    ResolvedModule, SourcePackage, TestTarget, TestTargetId,
};
pub(crate) use modules::resolve_explicit_module_path;
pub(crate) use overlay::PackageSourceOverlay;
pub(crate) use targets::target_module_at_offset;
