use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_model::CompilationTarget;

/// One package whose exact identity, physical root, and dependency aliases were resolved before
/// source discovery.
#[derive(Clone, Debug)]
pub struct ResolvedPackage {
    identity: PackageIdentity,
    display_name: Box<str>,
    root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
}

impl ResolvedPackage {
    #[must_use]
    pub fn new(
        identity: PackageIdentity,
        display_name: impl Into<Box<str>>,
        root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            identity,
            display_name: display_name.into(),
            root: root.into(),
            dependencies: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_dependency(mut self, alias: impl Into<Box<str>>, package: PackageIdentity) -> Self {
        self.dependencies.insert(alias.into(), package);
        self
    }

    #[must_use]
    pub const fn identity(&self) -> &PackageIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<Box<str>, PackageIdentity> {
        &self.dependencies
    }
}

/// Closed package graph and initial directory modules selected for one compile unit.
#[derive(Debug)]
pub struct DiscoveryRequest {
    target: CompilationTarget,
    packages: Vec<ResolvedPackage>,
    roots: Vec<ModuleIdentity>,
}

impl DiscoveryRequest {
    #[must_use]
    pub fn new(
        target: CompilationTarget,
        packages: Vec<ResolvedPackage>,
        roots: Vec<ModuleIdentity>,
    ) -> Self {
        Self {
            target,
            packages,
            roots,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn packages(&self) -> &[ResolvedPackage] {
        &self.packages
    }

    #[must_use]
    pub fn roots(&self) -> &[ModuleIdentity] {
        &self.roots
    }

    pub(crate) fn into_parts(
        self,
    ) -> (CompilationTarget, Vec<ResolvedPackage>, Vec<ModuleIdentity>) {
        (self.target, self.packages, self.roots)
    }
}
