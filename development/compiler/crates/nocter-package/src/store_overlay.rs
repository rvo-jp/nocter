use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_package_cache::VerifiedExactPackageRoot;

use crate::PackageId;

/// Exact package roots staged by a package-state transaction before store publication.
///
/// Resolution checks this immutable overlay before either persistent store. The overlay does not
/// grant the resolver authority to create, move, or remove a package directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageStoreOverlay {
    packages: BTreeMap<PackageId, VerifiedExactPackageRoot>,
}

impl PackageStoreOverlay {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
        }
    }

    /// Adds one staged exact package without permitting its root to change.
    ///
    /// Repeating the same root is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error when the package already maps to a different root.
    pub fn insert(
        &mut self,
        package: PackageId,
        root: VerifiedExactPackageRoot,
    ) -> Result<(), PackageStoreOverlayError> {
        if let Some(first) = self.packages.get(&package) {
            if first != &root {
                return Err(PackageStoreOverlayError {
                    package,
                    first: first.as_path().into(),
                    second: root.into_path(),
                });
            }
            return Ok(());
        }
        self.packages.insert(package, root);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, package: &PackageId) -> Option<&Path> {
        self.packages
            .get(package)
            .map(VerifiedExactPackageRoot::as_path)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageStoreOverlayError {
    package: PackageId,
    first: PathBuf,
    second: PathBuf,
}

impl PackageStoreOverlayError {
    #[must_use]
    pub const fn package(&self) -> &PackageId {
        &self.package
    }

    #[must_use]
    pub fn first(&self) -> &Path {
        &self.first
    }

    #[must_use]
    pub fn second(&self) -> &Path {
        &self.second
    }
}

impl fmt::Display for PackageStoreOverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "exact package {} is staged at both {} and {}",
            self.package.as_str(),
            self.first.display(),
            self.second.display()
        )
    }
}

impl std::error::Error for PackageStoreOverlayError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    fn sealed_root(name: &str, package: &PackageId) -> VerifiedExactPackageRoot {
        let root = std::env::temp_dir().join(format!(
            "nocter-package-store-overlay-{}-{}-{name}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("index.nct"),
            "#package: { name: \"fixture\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        nocter_package_cache::seal_exact_package(&root, package.as_str()).unwrap()
    }

    #[test]
    fn repeated_root_is_idempotent_but_relocation_is_rejected() {
        let package =
            PackageId::from_git_commit("7db21c1000000000000000000000000000000000").unwrap();
        let first = sealed_root("first", &package);
        let second = sealed_root("second", &package);
        let mut overlay = PackageStoreOverlay::new();

        overlay.insert(package.clone(), first.clone()).unwrap();
        overlay.insert(package.clone(), first.clone()).unwrap();
        assert!(matches!(
            overlay.insert(package, second.clone()),
            Err(PackageStoreOverlayError { .. })
        ));
        fs::remove_dir_all(first.as_path()).unwrap();
        fs::remove_dir_all(second.as_path()).unwrap();
    }
}
