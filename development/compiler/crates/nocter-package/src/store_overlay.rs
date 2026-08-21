use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::PackageId;

/// Exact package roots staged by a package-state transaction before store publication.
///
/// Resolution checks this immutable overlay before either persistent store. The overlay does not
/// grant the resolver authority to create, move, or remove a package directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageStoreOverlay {
    packages: BTreeMap<PackageId, PathBuf>,
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
        root: impl Into<PathBuf>,
    ) -> Result<(), PackageStoreOverlayError> {
        let root = root.into();
        if let Some(first) = self.packages.get(&package) {
            if first != &root {
                return Err(PackageStoreOverlayError {
                    package,
                    first: first.clone(),
                    second: root,
                });
            }
            return Ok(());
        }
        self.packages.insert(package, root);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, package: &PackageId) -> Option<&Path> {
        self.packages.get(package).map(PathBuf::as_path)
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
    use super::*;

    #[test]
    fn repeated_root_is_idempotent_but_relocation_is_rejected() {
        let package =
            PackageId::from_git_commit("7db21c1000000000000000000000000000000000").unwrap();
        let mut overlay = PackageStoreOverlay::new();

        overlay
            .insert(package.clone(), "/work/staging/first")
            .unwrap();
        overlay
            .insert(package.clone(), "/work/staging/first")
            .unwrap();
        assert!(matches!(
            overlay.insert(package, "/work/staging/second"),
            Err(PackageStoreOverlayError { .. })
        ));
    }
}
