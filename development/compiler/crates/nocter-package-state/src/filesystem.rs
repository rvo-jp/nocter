use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn ensure_directory(path: &Path) -> Result<PathBuf, PackageStateFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(invalid("select package-state directory", path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_directory(path)?,
        Err(error) => {
            return Err(PackageStateFilesystemError::new(
                "inspect package-state directory",
                path,
                error,
            ));
        }
    }
    fs::canonicalize(path).map_err(|error| {
        PackageStateFilesystemError::new("canonicalize package-state directory", path, error)
    })
}

pub(crate) fn ensure_contained(
    root: &Path,
    path: &Path,
) -> Result<(), PackageStateFilesystemError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(invalid("contain package-state directory", path))
    }
}

pub(crate) fn create_directory(path: &Path) -> Result<(), PackageStateFilesystemError> {
    fs::create_dir(path).map_err(|error| {
        PackageStateFilesystemError::new("create package-state directory", path, error)
    })
}

pub(crate) fn validate_physical_package_root(
    root: &Path,
) -> Result<(), PackageStateFilesystemError> {
    require_physical_directory(root)?;
    require_physical_file(&root.join("index.nct"))
}

fn require_physical_directory(path: &Path) -> Result<(), PackageStateFilesystemError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PackageStateFilesystemError::new("inspect exact package", path, error))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid("validate exact package directory", path))
    }
}

fn require_physical_file(path: &Path) -> Result<(), PackageStateFilesystemError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PackageStateFilesystemError::new("inspect exact package file", path, error)
    })?;
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid("validate exact package file", path))
    }
}

pub(crate) fn invalid(operation: &'static str, path: &Path) -> PackageStateFilesystemError {
    PackageStateFilesystemError::new(
        operation,
        path,
        io::Error::new(io::ErrorKind::InvalidData, "invalid package-state object"),
    )
}

#[derive(Debug)]
pub struct PackageStateFilesystemError {
    operation: &'static str,
    path: PathBuf,
    error: io::Error,
}

impl PackageStateFilesystemError {
    pub(crate) fn new(operation: &'static str, path: impl Into<PathBuf>, error: io::Error) -> Self {
        Self {
            operation,
            path: path.into(),
            error,
        }
    }
}

impl fmt::Display for PackageStateFilesystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot {} {}: {}",
            self.operation,
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for PackageStateFilesystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
