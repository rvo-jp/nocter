//! Package-state coordination above read-only exact graph resolution.
//!
//! Acquisition is injected explicitly. This crate owns staging, append-only exact-package cache
//! publication, graph revalidation, and failure-atomic generated exact-selection commit; it does
//! not implement a transport protocol.

mod authority;
mod filesystem;
mod package_cache;
mod root_source;
mod staging;
mod transaction;

pub use authority::{LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest};
pub use filesystem::PackageStateFilesystemError;
pub use root_source::RootSourceCommitError;
pub use transaction::{
    PackageFilesystemRevision, PackageFilesystemRevisionError, PackageResolutionAttemptError,
    PackageResolutionDriver, PackageStateError, resolve_package_state_with_driver,
};

#[cfg(test)]
mod tests;
