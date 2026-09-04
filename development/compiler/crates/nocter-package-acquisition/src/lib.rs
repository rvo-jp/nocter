//! Embedded HTTPS acquisition for exact Git and `.tar.gz` Nocter packages.
//!
//! This crate owns transport and materialization policy. Package graph validation, publication,
//! and generated exact-selection commits remain in `nocter-package-state`.

mod archive;
mod error;
mod git;
mod http;
mod policy;

use std::collections::BTreeMap;
use std::path::PathBuf;

use nocter_package::{DependencySource, ExactDependencyLock};
use nocter_package_state::{
    LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest,
};

pub use error::PackageAcquisitionError;

use crate::archive::{archive_lock, extract_archive, verified_archive};
use crate::git::{clone_repository, export_commit, resolve_revision};
use crate::http::HttpsClient;

#[derive(Clone, Debug)]
enum CachedArtifact {
    Archive(PathBuf),
    Git(PathBuf),
}

/// The in-process public HTTPS package acquisition authority.
pub struct EmbeddedPackageAcquisition {
    http: HttpsClient,
    cache: BTreeMap<ExactDependencyLock, CachedArtifact>,
}

impl EmbeddedPackageAcquisition {
    /// Builds an authority with the fixed TLS, redirect, and resource policy.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded HTTPS client cannot be initialized.
    pub fn new() -> Result<Self, PackageAcquisitionError> {
        Ok(Self {
            http: HttpsClient::new()?,
            cache: BTreeMap::new(),
        })
    }

    fn resolve_archive(
        &mut self,
        url: &str,
        workspace: &std::path::Path,
    ) -> Result<ExactDependencyLock, PackageAcquisitionError> {
        let bytes = self.http.download_archive(url)?;
        let lock = archive_lock(&bytes)?;
        let artifact = workspace.join("package.tar.gz");
        std::fs::write(&artifact, bytes).map_err(|error| {
            PackageAcquisitionError::filesystem("write archive cache", &artifact, error)
        })?;
        self.cache
            .insert(lock.clone(), CachedArtifact::Archive(artifact));
        Ok(lock)
    }

    fn fetch_archive(
        &mut self,
        url: &str,
        lock: &ExactDependencyLock,
        destination: &std::path::Path,
        workspace: &std::path::Path,
    ) -> Result<(), PackageAcquisitionError> {
        let cached = match self.cache.get(lock) {
            Some(CachedArtifact::Archive(path)) => Some(path.clone()),
            Some(CachedArtifact::Git(_)) | None => None,
        };
        let artifact = if let Some(path) = cached {
            path
        } else {
            let bytes = self.http.download_archive(url)?;
            let path = workspace.join("package.tar.gz");
            std::fs::write(&path, bytes).map_err(|error| {
                PackageAcquisitionError::filesystem("write archive workspace", &path, error)
            })?;
            path
        };
        let bytes = std::fs::read(&artifact).map_err(|error| {
            PackageAcquisitionError::filesystem("read archive workspace", &artifact, error)
        })?;
        verified_archive(&bytes, lock)?;
        extract_archive(&bytes, destination)
    }

    fn resolve_git(
        &mut self,
        url: &str,
        revision: &str,
        workspace: &std::path::Path,
    ) -> Result<ExactDependencyLock, PackageAcquisitionError> {
        let repository = workspace.join("repository.git");
        clone_repository(url, &repository)?;
        let lock = resolve_revision(&repository, revision)?;
        self.cache
            .insert(lock.clone(), CachedArtifact::Git(repository));
        Ok(lock)
    }

    fn fetch_git(
        &mut self,
        url: &str,
        lock: &ExactDependencyLock,
        destination: &std::path::Path,
        workspace: &std::path::Path,
    ) -> Result<(), PackageAcquisitionError> {
        let cached = match self.cache.get(lock) {
            Some(CachedArtifact::Git(path)) => Some(path.clone()),
            Some(CachedArtifact::Archive(_)) | None => None,
        };
        let repository = if let Some(path) = cached {
            path
        } else {
            let path = workspace.join("repository.git");
            clone_repository(url, &path)?;
            path
        };
        export_commit(&repository, lock, destination)
    }
}

impl PackageAcquisitionAuthority for EmbeddedPackageAcquisition {
    type Error = PackageAcquisitionError;

    fn resolve_lock(
        &mut self,
        request: LockResolutionRequest<'_>,
    ) -> Result<ExactDependencyLock, Self::Error> {
        match request.source() {
            DependencySource::Git { url, revision } => {
                self.resolve_git(url.value(), revision.value(), request.workspace())
            }
            DependencySource::Archive { url } => {
                self.resolve_archive(url.value(), request.workspace())
            }
            DependencySource::Path { .. } => Err(PackageAcquisitionError::unsupported(
                "path dependencies do not require remote lock resolution",
            )),
        }
    }

    fn fetch_package(&mut self, request: PackageFetchRequest<'_>) -> Result<(), Self::Error> {
        match request.source() {
            DependencySource::Git { url, .. } => self.fetch_git(
                url.value(),
                request.lock(),
                request.destination(),
                request.workspace(),
            ),
            DependencySource::Archive { url } => self.fetch_archive(
                url.value(),
                request.lock(),
                request.destination(),
                request.workspace(),
            ),
            DependencySource::Path { .. } => Err(PackageAcquisitionError::unsupported(
                "path dependencies are not fetched into an exact-package store",
            )),
        }
    }
}
