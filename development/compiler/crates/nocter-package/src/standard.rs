use std::path::{Path, PathBuf};

use nocter_model::PackageIdentity;

/// Exact standard-library package selected by the active toolchain.
#[derive(Clone, Debug)]
pub struct StandardPackage {
    identity: PackageIdentity,
    root: PathBuf,
    release: Box<str>,
}

impl StandardPackage {
    /// Declared package name required of the selected standard package.
    pub const DECLARED_NAME: &'static str = "std";

    /// Dependency alias under which every package reaches the selected standard package.
    pub const DEPENDENCY_ALIAS: &'static str = Self::DECLARED_NAME;

    #[must_use]
    pub fn new(
        identity: PackageIdentity,
        root: impl Into<PathBuf>,
        release: impl Into<Box<str>>,
    ) -> Self {
        Self {
            identity,
            root: root.into(),
            release: release.into(),
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &PackageIdentity {
        &self.identity
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn release(&self) -> &str {
        &self.release
    }

    pub(crate) fn into_parts(self) -> (PackageIdentity, PathBuf, Box<str>) {
        (self.identity, self.root, self.release)
    }
}
