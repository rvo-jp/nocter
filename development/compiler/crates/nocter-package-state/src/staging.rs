use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_package::PackageId;

static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(0);

pub(crate) struct StagingArea {
    root: PathBuf,
    transactions: PathBuf,
    state: PathBuf,
    packages: BTreeMap<PackageId, PathBuf>,
}

impl StagingArea {
    pub(crate) fn new(package_root: &Path) -> Result<Self, PackageStateFilesystemError> {
        let state = ensure_directory(&package_root.join(".nocter"))?;
        ensure_contained(package_root, &state)?;
        let transactions = ensure_directory(&state.join("transactions"))?;
        let root = create_unique_directory(&transactions)?;
        Ok(Self {
            root,
            transactions,
            state,
            packages: BTreeMap::new(),
        })
    }

    pub(crate) fn create_package(
        &mut self,
        package: PackageId,
    ) -> Result<PathBuf, PackageStateFilesystemError> {
        let path = self.root.join(package.as_str());
        create_directory(&path)?;
        self.packages.insert(package, path.clone());
        Ok(path)
    }

    pub(crate) fn validate_package(
        &self,
        package: &PackageId,
    ) -> Result<(), PackageStateFilesystemError> {
        let root = self
            .packages
            .get(package)
            .ok_or_else(|| invalid("validate unknown staged package", &self.root))?;
        require_physical_directory(root)?;
        require_physical_file(&root.join("nocter.nct"))
    }

    pub(crate) fn publish(
        &mut self,
        package_root: &Path,
    ) -> Result<(), PackageStateFilesystemError> {
        let state = ensure_directory(&package_root.join(".nocter"))?;
        ensure_contained(package_root, &state)?;
        let store = ensure_directory(&state.join("packages"))?;
        for (package, staged) in std::mem::take(&mut self.packages) {
            let destination = store.join(package.as_str());
            match fs::symlink_metadata(&destination) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => continue,
                Ok(_) => return Err(invalid("publish exact package", &destination)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(PackageStateFilesystemError::new(
                        "inspect exact package",
                        destination,
                        error,
                    ));
                }
            }
            fs::rename(&staged, &destination).map_err(|error| {
                PackageStateFilesystemError::new("publish exact package", &destination, error)
            })?;
        }
        Ok(())
    }
}

impl Drop for StagingArea {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir(&self.transactions);
        let _ = fs::remove_dir(&self.state);
    }
}

fn ensure_directory(path: &Path) -> Result<PathBuf, PackageStateFilesystemError> {
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

fn ensure_contained(root: &Path, path: &Path) -> Result<(), PackageStateFilesystemError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(invalid("contain package-state directory", path))
    }
}

fn create_unique_directory(parent: &Path) -> Result<PathBuf, PackageStateFilesystemError> {
    for _ in 0..128 {
        let serial = NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{}-{serial}", std::process::id()));
        match create_private_directory(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(PackageStateFilesystemError::new(
                    "create package transaction",
                    path,
                    error,
                ));
            }
        }
    }
    Err(invalid("create unique package transaction", parent))
}

fn create_directory(path: &Path) -> Result<(), PackageStateFilesystemError> {
    fs::create_dir(path).map_err(|error| {
        PackageStateFilesystemError::new("create package-state directory", path, error)
    })
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

fn require_physical_directory(path: &Path) -> Result<(), PackageStateFilesystemError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PackageStateFilesystemError::new("inspect staged package", path, error))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid("validate staged package directory", path))
    }
}

fn require_physical_file(path: &Path) -> Result<(), PackageStateFilesystemError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PackageStateFilesystemError::new("inspect staged package file", path, error)
    })?;
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid("validate staged package file", path))
    }
}

fn invalid(operation: &'static str, path: &Path) -> PackageStateFilesystemError {
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
