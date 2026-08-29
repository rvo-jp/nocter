use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_package::PackageLockSourceUpdate;

use crate::filesystem::PackageStateFilesystemError;

static NEXT_SOURCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn commit_root_lock_source(
    update: &PackageLockSourceUpdate,
) -> Result<(), RootSourceCommitError> {
    let path = update.path();
    let current = fs::read(path).map_err(|error| filesystem("read package source", path, error))?;
    if current != update.original() {
        return Err(RootSourceCommitError::SourceChanged(path.into()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| RootSourceCommitError::Filesystem(invalid(path)))?;
    let (temporary, mut file) = create_unique_file(parent)?;
    let result = write_and_replace(&mut file, update.replacement(), &temporary, path);
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_and_replace(
    file: &mut File,
    bytes: &[u8],
    temporary: &Path,
    destination: &Path,
) -> Result<(), RootSourceCommitError> {
    file.write_all(bytes)
        .map_err(|error| filesystem("write generated lock source", temporary, error))?;
    let permissions = fs::metadata(destination)
        .map_err(|error| filesystem("read package source metadata", destination, error))?
        .permissions();
    file.set_permissions(permissions)
        .map_err(|error| filesystem("preserve package source permissions", temporary, error))?;
    file.sync_all()
        .map_err(|error| filesystem("synchronize generated lock source", temporary, error))?;
    fs::rename(temporary, destination)
        .map_err(|error| filesystem("commit generated lock source", destination, error))
}

fn create_unique_file(parent: &Path) -> Result<(PathBuf, File), RootSourceCommitError> {
    for _ in 0..128 {
        let serial = NEXT_SOURCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".index.nct.{}-{serial}.tmp", std::process::id()));
        match open_private_file(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(filesystem("create generated lock source", path, error)),
        }
    }
    Err(RootSourceCommitError::Filesystem(invalid(parent)))
}

#[cfg(unix)]
fn open_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn invalid(path: &Path) -> PackageStateFilesystemError {
    PackageStateFilesystemError::new(
        "select generated lock source",
        path,
        io::Error::new(io::ErrorKind::InvalidInput, "invalid package source path"),
    )
}

fn filesystem(
    operation: &'static str,
    path: impl Into<PathBuf>,
    error: io::Error,
) -> RootSourceCommitError {
    RootSourceCommitError::Filesystem(PackageStateFilesystemError::new(operation, path, error))
}

#[derive(Debug)]
pub enum RootSourceCommitError {
    SourceChanged(PathBuf),
    Filesystem(PackageStateFilesystemError),
}

impl std::fmt::Display for RootSourceCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceChanged(path) => write!(
                formatter,
                "package source changed during dependency resolution: {}",
                path.display()
            ),
            Self::Filesystem(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RootSourceCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem(error) => Some(error),
            Self::SourceChanged(_) => None,
        }
    }
}
