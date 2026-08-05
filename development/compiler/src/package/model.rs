use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageId(String);

impl PackageId {
    pub(super) fn root(root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        Self::from_descriptor(&format!("root:{}", root.display()))
    }

    pub(super) fn from_descriptor(descriptor: &str) -> Self {
        let digest = Sha256::digest(descriptor.as_bytes());
        Self(format!("{digest:x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId {
    package: PackageId,
    logical_path: String,
}

impl ModuleId {
    pub(super) fn new(package: PackageId, logical_path: String) -> Self {
        Self {
            package,
            logical_path,
        }
    }

    pub fn package(&self) -> &PackageId {
        &self.package
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutableId {
    package: PackageId,
    name: String,
}

impl ExecutableId {
    pub(super) fn new(package: PackageId, name: String) -> Self {
        Self { package, name }
    }

    pub fn package(&self) -> &PackageId {
        &self.package
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTarget {
    id: ExecutableId,
    name: String,
    module: ModuleId,
    source_path: PathBuf,
}

impl ExecutableTarget {
    pub(super) fn new(
        id: ExecutableId,
        name: String,
        module: ModuleId,
        source_path: PathBuf,
    ) -> Self {
        Self {
            id,
            name,
            module,
            source_path,
        }
    }

    pub fn id(&self) -> &ExecutableId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn module(&self) -> &ModuleId {
        &self.module
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePackage {
    id: PackageId,
    root: PathBuf,
    manifest_path: PathBuf,
    display_name: String,
    version: Option<String>,
    dependencies: Vec<super::DependencyDeclaration>,
    locks: Vec<super::LockedDependency>,
    executables: Vec<ExecutableTarget>,
}

impl SourcePackage {
    pub(super) fn new(
        id: PackageId,
        root: PathBuf,
        manifest_path: PathBuf,
        display_name: String,
        version: Option<String>,
        dependencies: Vec<super::DependencyDeclaration>,
        locks: Vec<super::LockedDependency>,
        executables: Vec<ExecutableTarget>,
    ) -> Self {
        Self {
            id,
            root,
            manifest_path,
            display_name,
            version,
            dependencies,
            locks,
            executables,
        }
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn dependencies(&self) -> &[super::DependencyDeclaration] {
        &self.dependencies
    }

    pub fn locks(&self) -> &[super::LockedDependency] {
        &self.locks
    }

    pub(super) fn replace_locks(&mut self, locks: Vec<super::LockedDependency>) {
        self.locks = locks;
    }

    pub fn lock(&self, name: &str) -> Option<&super::DependencyLock> {
        self.locks
            .iter()
            .find(|lock| lock.name() == name)
            .map(super::LockedDependency::resolution)
    }

    pub fn executables(&self) -> &[ExecutableTarget] {
        &self.executables
    }

    pub fn executable(&self, name: &str) -> Option<&ExecutableTarget> {
        self.executables.iter().find(|target| target.name == name)
    }
}
