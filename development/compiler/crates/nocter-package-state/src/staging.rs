use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_package::PackageId;

use crate::filesystem::{
    PackageStateFilesystemError, create_directory, ensure_contained, ensure_directory, invalid,
    validate_physical_package_root,
};

static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(0);

pub(crate) struct StagingArea {
    root: PathBuf,
    transactions: PathBuf,
    state: PathBuf,
    packages: BTreeMap<PackageId, PathBuf>,
    next_workspace: u64,
}

pub(crate) struct StagedPackages(BTreeMap<PackageId, PathBuf>);

impl StagedPackages {
    pub(crate) fn into_entries(self) -> impl Iterator<Item = (PackageId, PathBuf)> {
        self.0.into_iter()
    }
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
            next_workspace: 0,
        })
    }

    pub(crate) fn create_workspace(&mut self) -> Result<PathBuf, PackageStateFilesystemError> {
        let workspaces = self.root.join("acquisition");
        if self.next_workspace == 0 {
            create_directory(&workspaces)?;
        }
        let path = workspaces.join(self.next_workspace.to_string());
        self.next_workspace += 1;
        create_directory(&path)?;
        Ok(path)
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
        validate_physical_package_root(root)
    }

    pub(crate) fn take_packages(&mut self) -> Option<StagedPackages> {
        (!self.packages.is_empty()).then(|| StagedPackages(std::mem::take(&mut self.packages)))
    }
}

impl Drop for StagingArea {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir(&self.transactions);
        let _ = fs::remove_dir(&self.state);
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
