use std::error::Error;
use std::path::Path;

use nocter_model::PackageIdentity;
use nocter_package::{DependencySource, ExactDependencyLock, PackageId};

/// One unresolved direct dependency presented to the selected acquisition implementation.
#[derive(Clone, Copy, Debug)]
pub struct LockResolutionRequest<'a> {
    package: &'a PackageIdentity,
    alias: &'a str,
    source: &'a DependencySource,
    workspace: &'a Path,
}

impl<'a> LockResolutionRequest<'a> {
    pub(crate) const fn new(
        package: &'a PackageIdentity,
        alias: &'a str,
        source: &'a DependencySource,
        workspace: &'a Path,
    ) -> Self {
        Self {
            package,
            alias,
            source,
            workspace,
        }
    }

    #[must_use]
    pub const fn package(self) -> &'a PackageIdentity {
        self.package
    }

    #[must_use]
    pub const fn alias(self) -> &'a str {
        self.alias
    }

    #[must_use]
    pub const fn source(self) -> &'a DependencySource {
        self.source
    }

    /// Returns an empty transaction-private directory for provisional acquisition data.
    #[must_use]
    pub const fn workspace(self) -> &'a Path {
        self.workspace
    }
}

/// One exact package presented with an empty private staging directory.
#[derive(Clone, Copy, Debug)]
pub struct PackageFetchRequest<'a> {
    package: &'a PackageIdentity,
    alias: &'a str,
    source: &'a DependencySource,
    lock: &'a ExactDependencyLock,
    package_id: &'a PackageId,
    destination: &'a Path,
    workspace: &'a Path,
}

impl<'a> PackageFetchRequest<'a> {
    pub(crate) const fn new(
        package: &'a PackageIdentity,
        alias: &'a str,
        source: &'a DependencySource,
        lock: &'a ExactDependencyLock,
        package_id: &'a PackageId,
        destination: &'a Path,
        workspace: &'a Path,
    ) -> Self {
        Self {
            package,
            alias,
            source,
            lock,
            package_id,
            destination,
            workspace,
        }
    }

    #[must_use]
    pub const fn package(self) -> &'a PackageIdentity {
        self.package
    }

    #[must_use]
    pub const fn alias(self) -> &'a str {
        self.alias
    }

    #[must_use]
    pub const fn source(self) -> &'a DependencySource {
        self.source
    }

    #[must_use]
    pub const fn lock(self) -> &'a ExactDependencyLock {
        self.lock
    }

    #[must_use]
    pub const fn package_id(self) -> &'a PackageId {
        self.package_id
    }

    #[must_use]
    pub const fn destination(self) -> &'a Path {
        self.destination
    }

    /// Returns an empty transaction-private directory for transport scratch data.
    #[must_use]
    pub const fn workspace(self) -> &'a Path {
        self.workspace
    }
}

/// Transport-specific authority used by one package-state transaction.
///
/// Implementations must resolve Git revisions to exact commits, verify archive digests, and place
/// only the requested package contents inside `destination`. Provisional downloads and repository
/// data belong in the supplied transaction-private workspace. The coordinator publishes nothing
/// until the staged graph is valid.
pub trait PackageAcquisitionAuthority {
    type Error: Error + Send + Sync + 'static;

    /// Resolves one remote source selection to the exact lock kind required by that source.
    ///
    /// # Errors
    ///
    /// Returns the transport authority's failure without changing coordinator-owned state.
    fn resolve_lock(
        &mut self,
        request: LockResolutionRequest<'_>,
    ) -> Result<ExactDependencyLock, Self::Error>;

    /// Populates the supplied empty private directory with one verified exact package.
    ///
    /// # Errors
    ///
    /// Returns the transport authority's failure. The coordinator removes unpublished staging
    /// state after the transaction ends.
    fn fetch_package(&mut self, request: PackageFetchRequest<'_>) -> Result<(), Self::Error>;
}
