use std::fs;
use std::io;
use std::path::Path;

use crate::filesystem::{
    PackageStateFilesystemError, ensure_contained, ensure_directory, validate_physical_package_root,
};
use crate::staging::StagedPackages;

/// Outcome of publishing validated exact packages into the append-only local cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackageCachePublication {
    requires_filesystem_refresh: bool,
}

impl PackageCachePublication {
    pub(crate) const fn requires_filesystem_refresh(self) -> bool {
        self.requires_filesystem_refresh
    }
}

/// Publishes validated exact-package identities independently from root-source selection.
///
/// Cache entries are immutable and carry no dependency-selection authority. Consequently an entry
/// may safely survive a later root exact-selection rejection. An existing physical directory wins
/// a concurrent publication race for the same exact identity.
pub(crate) fn publish_exact_packages(
    package_root: &Path,
    packages: StagedPackages,
) -> Result<PackageCachePublication, PackageStateFilesystemError> {
    let state = ensure_directory(&package_root.join(".nocter"))?;
    ensure_contained(package_root, &state)?;
    let store = ensure_directory(&state.join("packages"))?;
    for (package, staged) in packages.into_entries() {
        let destination = store.join(package.as_str());
        if physical_package_exists(&destination)? {
            continue;
        }
        match fs::rename(&staged, &destination) {
            Ok(()) => {}
            Err(rename_error) => {
                if physical_package_exists(&destination)? {
                    continue;
                }
                return Err(PackageStateFilesystemError::new(
                    "publish exact package",
                    destination,
                    rename_error,
                ));
            }
        }
    }
    Ok(PackageCachePublication {
        // Resolution is about to replace its staged-store overlay with the physical cache. This
        // refresh is also required when another publisher won the destination race: that physical
        // entry was not part of the preceding filesystem view.
        requires_filesystem_refresh: true,
    })
}

fn physical_package_exists(path: &Path) -> Result<bool, PackageStateFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            validate_physical_package_root(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PackageStateFilesystemError::new(
            "inspect exact package",
            path,
            error,
        )),
    }
}
