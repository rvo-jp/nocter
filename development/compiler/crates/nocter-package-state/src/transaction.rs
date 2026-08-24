use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use nocter_model::PackageIdentity;
use nocter_package::{
    DependencySource, ExactDependencyLockKind, PackageId, PackageIdError, PackageLockOverlay,
    PackageLockOverlayError, PackageResolutionError, PackageResolutionRequest, PackageStoreOverlay,
    PackageStoreOverlayError, ResolvedPackageSelection, resolve_package_selection,
};

use crate::authority::{LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest};
use crate::root_source::{RootSourceCommitError, commit_root_lock_source};
use crate::staging::{PackageStateFilesystemError, StagingArea};

/// Resolves and commits all mutable package state as one graph-validated transaction.
///
/// Generated locks apply only to the selected root package. Exact packages are first acquired into
/// a private store overlay. The complete staged graph must resolve before any package is published;
/// persistent stores are then selected again before the root lock source is committed atomically.
///
/// # Errors
///
/// Returns a typed resolution, acquisition, staging, publication, or source-commit failure.
pub fn resolve_package_state<A: PackageAcquisitionAuthority>(
    request: PackageResolutionRequest,
    authority: &mut A,
) -> Result<ResolvedPackageSelection, PackageStateError<A::Error>> {
    PackageStateTransaction::new(request)?.run(authority)
}

struct PackageStateTransaction {
    request: PackageResolutionRequest,
    canonical_root: PathBuf,
    root: PackageIdentity,
    locks: PackageLockOverlay,
    store: PackageStoreOverlay,
    staging: Option<StagingArea>,
    generated_locks: bool,
    acquired_packages: bool,
}

impl PackageStateTransaction {
    fn new<E: Error + Send + Sync + 'static>(
        request: PackageResolutionRequest,
    ) -> Result<Self, PackageStateError<E>> {
        let canonical_root = fs::canonicalize(request.root()).map_err(|error| {
            PackageStateError::Filesystem(PackageStateFilesystemError::new(
                "canonicalize package root",
                request.root(),
                error,
            ))
        })?;
        let root = PackageId::from_canonical_path(&canonical_root)
            .map_err(PackageStateError::PackageId)?
            .package_identity();
        Ok(Self {
            request,
            canonical_root,
            root,
            locks: PackageLockOverlay::new(),
            store: PackageStoreOverlay::new(),
            staging: None,
            generated_locks: false,
            acquired_packages: false,
        })
    }

    fn run<A: PackageAcquisitionAuthority>(
        mut self,
        authority: &mut A,
    ) -> Result<ResolvedPackageSelection, PackageStateError<A::Error>> {
        loop {
            let attempt = self
                .request
                .clone()
                .with_lock_overlay(self.locks.clone())
                .with_store_overlay(self.store.clone());
            match resolve_package_selection(attempt) {
                Ok(selection) => return self.complete(selection),
                Err(PackageResolutionError::LockRequired {
                    package,
                    package_root,
                    alias,
                    source,
                }) => self.resolve_lock(authority, &package, package_root, &alias, &source)?,
                Err(PackageResolutionError::FetchRequired {
                    package,
                    alias,
                    package_id,
                    lock,
                    source,
                }) => self.fetch(authority, &package, &alias, package_id, &lock, &source)?,
                Err(error) => return Err(PackageStateError::Resolution(Box::new(error))),
            }
        }
    }

    fn resolve_lock<A: PackageAcquisitionAuthority>(
        &mut self,
        authority: &mut A,
        package: &PackageIdentity,
        package_root: PathBuf,
        alias: &str,
        source: &DependencySource,
    ) -> Result<(), PackageStateError<A::Error>> {
        if package != &self.root {
            return Err(PackageStateError::NonRootLockRequired {
                root: self.root.clone(),
                package: package.clone(),
                package_root,
                alias: alias.into(),
            });
        }
        let workspace = self
            .staging_area()?
            .create_workspace()
            .map_err(PackageStateError::Filesystem)?;
        let lock = authority
            .resolve_lock(LockResolutionRequest::new(
                package, alias, source, &workspace,
            ))
            .map_err(PackageStateError::Acquisition)?;
        let expected =
            source_lock_kind(source).ok_or_else(|| PackageStateError::UnexpectedLockSource {
                package: package.clone(),
                alias: alias.into(),
            })?;
        if lock.kind() != expected {
            return Err(PackageStateError::LockKindMismatch {
                package: package.clone(),
                alias: alias.into(),
                expected,
                actual: lock.kind(),
            });
        }
        self.locks
            .insert(self.root.clone(), alias, lock)
            .map_err(PackageStateError::LockOverlay)?;
        self.generated_locks = true;
        Ok(())
    }

    fn fetch<A: PackageAcquisitionAuthority>(
        &mut self,
        authority: &mut A,
        package: &PackageIdentity,
        alias: &str,
        package_id: PackageId,
        lock: &nocter_package::ExactDependencyLock,
        source: &DependencySource,
    ) -> Result<(), PackageStateError<A::Error>> {
        let area = self.staging_area()?;
        let destination = area
            .create_package(package_id.clone())
            .map_err(PackageStateError::Filesystem)?;
        let workspace = area
            .create_workspace()
            .map_err(PackageStateError::Filesystem)?;
        authority
            .fetch_package(PackageFetchRequest::new(
                package,
                alias,
                source,
                lock,
                &package_id,
                &destination,
                &workspace,
            ))
            .map_err(PackageStateError::Acquisition)?;
        area.validate_package(&package_id)
            .map_err(PackageStateError::Filesystem)?;
        self.store
            .insert(package_id, destination)
            .map_err(PackageStateError::StoreOverlay)?;
        self.acquired_packages = true;
        Ok(())
    }

    fn staging_area<E: Error + Send + Sync + 'static>(
        &mut self,
    ) -> Result<&mut StagingArea, PackageStateError<E>> {
        if self.staging.is_none() {
            let created =
                StagingArea::new(&self.canonical_root).map_err(PackageStateError::Filesystem)?;
            self.staging = Some(created);
        }
        Ok(self.staging.as_mut().expect("staging area was initialized"))
    }

    fn complete<E: Error + Send + Sync + 'static>(
        mut self,
        selection: ResolvedPackageSelection,
    ) -> Result<ResolvedPackageSelection, PackageStateError<E>> {
        if !self.generated_locks && !self.acquired_packages {
            return Ok(selection);
        }
        if let Some(area) = &mut self.staging {
            area.publish(&self.canonical_root)
                .map_err(PackageStateError::Filesystem)?;
        }
        let selected =
            resolve_package_selection(self.request.clone().with_lock_overlay(self.locks.clone()))
                .map_err(|error| PackageStateError::Resolution(Box::new(error)))?;
        if self.generated_locks {
            let update = selected
                .graph()
                .root_lock_update(&self.root)
                .map_err(PackageStateError::LockSource)?;
            commit_root_lock_source(&update).map_err(PackageStateError::RootSourceCommit)?;
        }
        Ok(selected)
    }
}

fn source_lock_kind(source: &DependencySource) -> Option<ExactDependencyLockKind> {
    match source {
        DependencySource::Git { .. } => Some(ExactDependencyLockKind::Git),
        DependencySource::Archive { .. } => Some(ExactDependencyLockKind::Sha256),
        DependencySource::Path { .. } => None,
    }
}

#[derive(Debug)]
pub enum PackageStateError<E: Error + Send + Sync + 'static> {
    Acquisition(E),
    Resolution(Box<PackageResolutionError>),
    NonRootLockRequired {
        root: PackageIdentity,
        package: PackageIdentity,
        package_root: PathBuf,
        alias: Box<str>,
    },
    UnexpectedLockSource {
        package: PackageIdentity,
        alias: Box<str>,
    },
    LockKindMismatch {
        package: PackageIdentity,
        alias: Box<str>,
        expected: ExactDependencyLockKind,
        actual: ExactDependencyLockKind,
    },
    PackageId(PackageIdError),
    LockOverlay(PackageLockOverlayError),
    StoreOverlay(PackageStoreOverlayError),
    LockSource(nocter_package::PackageLockSourceError),
    RootSourceCommit(RootSourceCommitError),
    Filesystem(PackageStateFilesystemError),
}

impl<E: Error + Send + Sync + 'static> fmt::Display for PackageStateError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquisition(error) => write!(formatter, "package acquisition failed: {error}"),
            Self::Resolution(error) => error.fmt(formatter),
            Self::NonRootLockRequired {
                package,
                package_root,
                alias,
                ..
            } => write!(
                formatter,
                "dependency {alias} of non-root package {} at {} requires a lock",
                package.as_str(),
                package_root.display()
            ),
            Self::UnexpectedLockSource { package, alias } => write!(
                formatter,
                "package {} dependency {alias} requested a lock for a non-remote source",
                package.as_str()
            ),
            Self::LockKindMismatch {
                package,
                alias,
                expected,
                actual,
            } => write!(
                formatter,
                "package {} dependency {alias} requires a {} lock, not a {} lock",
                package.as_str(),
                expected.prefix(),
                actual.prefix()
            ),
            Self::PackageId(error) => error.fmt(formatter),
            Self::LockOverlay(error) => error.fmt(formatter),
            Self::StoreOverlay(error) => error.fmt(formatter),
            Self::LockSource(error) => error.fmt(formatter),
            Self::RootSourceCommit(error) => error.fmt(formatter),
            Self::Filesystem(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + Send + Sync + 'static> Error for PackageStateError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Acquisition(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::PackageId(error) => Some(error),
            Self::LockOverlay(error) => Some(error),
            Self::StoreOverlay(error) => Some(error),
            Self::LockSource(error) => Some(error),
            Self::RootSourceCommit(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            Self::NonRootLockRequired { .. }
            | Self::UnexpectedLockSource { .. }
            | Self::LockKindMismatch { .. } => None,
        }
    }
}
