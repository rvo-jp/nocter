use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use nocter_model::PackageIdentity;
use nocter_package::{
    DependencySource, ExactDependencyLockKind, PackageId, PackageIdError, PackageLockOverlay,
    PackageLockOverlayError, PackageResolutionError, PackageResolutionRequest, PackageStoreOverlay,
    PackageStoreOverlayError, ResolvedPackageSelection,
};

use crate::authority::{LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest};
use crate::root_source::{RootSourceCommitError, commit_root_lock_source};
use crate::staging::{PackageStateFilesystemError, StagingArea};

/// Read-only package-graph authority used by one mutable package-state transaction.
pub trait PackageResolutionDriver {
    /// Resolves one immutable package graph attempt through the driver's source authority.
    ///
    /// # Errors
    ///
    /// Returns either the authored package-domain rejection or an infrastructure failure from the
    /// injected source/query authority.
    fn resolve(
        &mut self,
        request: PackageResolutionRequest,
        filesystem_revision: PackageFilesystemRevision,
    ) -> Result<ResolvedPackageSelection, PackageResolutionAttemptError>;
}

/// Monotonic identity of filesystem mutations committed by one package-state transaction.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PackageFilesystemRevision(u64);

impl PackageFilesystemRevision {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn advance(&mut self) -> Result<(), PackageFilesystemRevisionError> {
        self.0 = self
            .0
            .checked_add(1)
            .ok_or(PackageFilesystemRevisionError)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageFilesystemRevisionError;

impl fmt::Display for PackageFilesystemRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("package filesystem revision identity space is exhausted")
    }
}

impl Error for PackageFilesystemRevisionError {}

/// Runs package-state coordination through a caller-owned source/query authority.
///
/// # Errors
///
/// Returns atomic transaction failures plus an infrastructure failure selected by `resolver`.
pub fn resolve_package_state_with_driver<
    A: PackageAcquisitionAuthority,
    R: PackageResolutionDriver,
>(
    request: PackageResolutionRequest,
    authority: &mut A,
    resolver: &mut R,
) -> Result<ResolvedPackageSelection, PackageStateError<A::Error>> {
    PackageStateTransaction::new(request)?.run(authority, resolver)
}

#[derive(Debug)]
pub enum PackageResolutionAttemptError {
    Domain(PackageResolutionError),
    Infrastructure(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for PackageResolutionAttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(formatter),
            Self::Infrastructure(error) => error.fmt(formatter),
        }
    }
}

impl Error for PackageResolutionAttemptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            Self::Infrastructure(error) => Some(&**error),
        }
    }
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
    filesystem_revision: PackageFilesystemRevision,
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
            filesystem_revision: PackageFilesystemRevision::default(),
        })
    }

    fn run<A: PackageAcquisitionAuthority, R: PackageResolutionDriver>(
        mut self,
        authority: &mut A,
        resolver: &mut R,
    ) -> Result<ResolvedPackageSelection, PackageStateError<A::Error>> {
        loop {
            let attempt = self
                .request
                .clone()
                .with_lock_overlay(self.locks.clone())
                .with_store_overlay(self.store.clone());
            match resolver.resolve(attempt, self.filesystem_revision) {
                Ok(selection) => return self.complete(selection, resolver),
                Err(PackageResolutionAttemptError::Domain(
                    PackageResolutionError::LockRequired {
                        package,
                        package_root,
                        alias,
                        source,
                    },
                )) => self.resolve_lock(authority, &package, package_root, &alias, &source)?,
                Err(PackageResolutionAttemptError::Domain(
                    PackageResolutionError::FetchRequired {
                        package,
                        alias,
                        package_id,
                        lock,
                        source,
                    },
                )) => self.fetch(authority, &package, &alias, package_id, &lock, &source)?,
                Err(PackageResolutionAttemptError::Domain(error)) => {
                    return Err(PackageStateError::Resolution(Box::new(error)));
                }
                Err(PackageResolutionAttemptError::Infrastructure(error)) => {
                    return Err(PackageStateError::ResolutionInfrastructure(error));
                }
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

    fn complete<E: Error + Send + Sync + 'static, R: PackageResolutionDriver>(
        mut self,
        selection: ResolvedPackageSelection,
        resolver: &mut R,
    ) -> Result<ResolvedPackageSelection, PackageStateError<E>> {
        if !self.generated_locks && !self.acquired_packages {
            return Ok(selection);
        }
        if let Some(area) = &mut self.staging {
            area.publish(&self.canonical_root)
                .map_err(PackageStateError::Filesystem)?;
        }
        if self.acquired_packages {
            self.filesystem_revision
                .advance()
                .map_err(PackageStateError::FilesystemRevision)?;
        }
        let selected = resolve_attempt(
            resolver,
            self.request.clone().with_lock_overlay(self.locks.clone()),
            self.filesystem_revision,
        )?;
        if self.generated_locks {
            let update = selected
                .graph()
                .root_lock_update(&self.root)
                .map_err(PackageStateError::LockSource)?;
            commit_root_lock_source(&update).map_err(PackageStateError::RootSourceCommit)?;
            self.filesystem_revision
                .advance()
                .map_err(PackageStateError::FilesystemRevision)?;
            return resolve_attempt(
                resolver,
                self.request.clone().with_lock_overlay(self.locks.clone()),
                self.filesystem_revision,
            );
        }
        Ok(selected)
    }
}

fn resolve_attempt<E: Error + Send + Sync + 'static, R: PackageResolutionDriver>(
    resolver: &mut R,
    request: PackageResolutionRequest,
    filesystem_revision: PackageFilesystemRevision,
) -> Result<ResolvedPackageSelection, PackageStateError<E>> {
    resolver
        .resolve(request, filesystem_revision)
        .map_err(|error| match error {
            PackageResolutionAttemptError::Domain(error) => {
                PackageStateError::Resolution(Box::new(error))
            }
            PackageResolutionAttemptError::Infrastructure(error) => {
                PackageStateError::ResolutionInfrastructure(error)
            }
        })
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
    ResolutionInfrastructure(Box<dyn Error + Send + Sync>),
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
    FilesystemRevision(PackageFilesystemRevisionError),
}

impl<E: Error + Send + Sync + 'static> fmt::Display for PackageStateError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquisition(error) => write!(formatter, "package acquisition failed: {error}"),
            Self::Resolution(error) => error.fmt(formatter),
            Self::ResolutionInfrastructure(error) => error.fmt(formatter),
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
            Self::FilesystemRevision(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + Send + Sync + 'static> Error for PackageStateError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Acquisition(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::ResolutionInfrastructure(error) => Some(&**error),
            Self::PackageId(error) => Some(error),
            Self::LockOverlay(error) => Some(error),
            Self::StoreOverlay(error) => Some(error),
            Self::LockSource(error) => Some(error),
            Self::RootSourceCommit(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            Self::FilesystemRevision(error) => Some(error),
            Self::NonRootLockRequired { .. }
            | Self::UnexpectedLockSource { .. }
            | Self::LockKindMismatch { .. } => None,
        }
    }
}
