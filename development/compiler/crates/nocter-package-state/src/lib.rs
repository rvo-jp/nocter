//! Failure-atomic package-state coordination above read-only exact graph resolution.
//!
//! Acquisition is injected explicitly. This crate owns staging, exact-store publication, graph
//! revalidation, and generated lock-source commit; it does not implement a transport protocol.

mod authority;
mod root_source;
mod staging;
mod transaction;

pub use authority::{LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest};
pub use root_source::RootSourceCommitError;
pub use staging::PackageStateFilesystemError;
pub use transaction::{PackageStateError, resolve_package_state};

#[cfg(test)]
mod tests;
