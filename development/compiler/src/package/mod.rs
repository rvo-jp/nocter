//! Source-native package loading and executable-target selection.

mod diagnostics;
mod loader;
mod location;
mod model;
mod validation;

#[cfg(test)]
mod tests;

pub use loader::{PackageLoad, load_package};
pub(crate) use location::validate_package_header_location;
pub use model::{ExecutableId, ExecutableTarget, ModuleId, PackageId, SourcePackage};
pub(crate) use validation::resolve_package_module;
