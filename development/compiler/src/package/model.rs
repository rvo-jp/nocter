use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageId(u32);

impl PackageId {
    pub(super) const ROOT: Self = Self(0);
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

    pub fn package(&self) -> PackageId {
        self.package
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

    pub fn package(&self) -> PackageId {
        self.package
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
    index_path: PathBuf,
    display_name: String,
    version: Option<String>,
    executables: Vec<ExecutableTarget>,
}

impl SourcePackage {
    pub(super) fn new(
        root: PathBuf,
        index_path: PathBuf,
        display_name: String,
        version: Option<String>,
        executables: Vec<ExecutableTarget>,
    ) -> Self {
        Self {
            id: PackageId::ROOT,
            root,
            index_path,
            display_name,
            version,
            executables,
        }
    }

    pub fn id(&self) -> PackageId {
        self.id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn executables(&self) -> &[ExecutableTarget] {
        &self.executables
    }

    pub fn executable(&self, name: &str) -> Option<&ExecutableTarget> {
        self.executables.iter().find(|target| target.name == name)
    }
}
