use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageId(String);

impl PackageId {
    pub(crate) fn root(root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        Self::from_descriptor(&format!("root:{}", root.display()))
    }

    pub(super) fn from_descriptor(descriptor: &str) -> Self {
        let digest = Sha256::digest(descriptor.as_bytes());
        Self(format!("{digest:x}"))
    }

    pub(crate) fn standard_library(root: &Path, version: Option<&str>) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        Self::from_descriptor(&format!(
            "toolchain-std:{}:{}",
            version.unwrap_or("unversioned"),
            root.display()
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId {
    package: PackageId,
    key: ModuleKey,
}

impl ModuleId {
    pub(crate) fn new(package: PackageId, key: ModuleKey) -> Self {
        Self { package, key }
    }

    pub fn package(&self) -> &PackageId {
        &self.package
    }

    pub fn key(&self) -> &ModuleKey {
        &self.key
    }

    pub(crate) fn contains(&self, module: &Self) -> bool {
        if self.package != module.package {
            return false;
        }
        match (&self.key, &module.key) {
            (ModuleKey::PackageRoot, _) => true,
            (ModuleKey::Path(_), ModuleKey::PackageRoot) => false,
            (ModuleKey::Path(boundary), ModuleKey::Path(candidate)) => {
                candidate.as_str() == boundary.as_str()
                    || candidate
                        .as_str()
                        .strip_prefix(boundary.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleKey {
    PackageRoot,
    Path(NormalizedModulePath),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedModulePath(String);

impl NormalizedModulePath {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    id: ModuleId,
    source_path: PathBuf,
}

impl ResolvedModule {
    pub(super) fn new(id: ModuleId, source_path: PathBuf) -> Self {
        Self { id, source_path }
    }

    pub fn id(&self) -> &ModuleId {
        &self.id
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
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
    module: ResolvedModule,
}

impl ExecutableTarget {
    pub(super) fn new(id: ExecutableId, module: ResolvedModule) -> Self {
        Self { id, module }
    }

    pub fn id(&self) -> &ExecutableId {
        &self.id
    }

    pub fn name(&self) -> &str {
        self.id.name()
    }

    pub fn module(&self) -> &ResolvedModule {
        &self.module
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestTargetId {
    package: PackageId,
    name: String,
}

impl TestTargetId {
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
pub struct TestTarget {
    id: TestTargetId,
    module: ResolvedModule,
}

impl TestTarget {
    pub(super) fn new(id: TestTargetId, module: ResolvedModule) -> Self {
        Self { id, module }
    }

    pub fn id(&self) -> &TestTargetId {
        &self.id
    }

    pub fn name(&self) -> &str {
        self.id.name()
    }

    pub fn module(&self) -> &ResolvedModule {
        &self.module
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePackage {
    id: PackageId,
    root: PathBuf,
    package_file_path: PathBuf,
    root_module: ResolvedModule,
    display_name: String,
    version: Option<String>,
    dependencies: Vec<super::DependencyDeclaration>,
    locks: Vec<super::LockedDependency>,
    executables: Vec<ExecutableTarget>,
    tests: Vec<TestTarget>,
}

impl SourcePackage {
    pub(super) fn new(
        id: PackageId,
        root: PathBuf,
        package_file_path: PathBuf,
        root_module: ResolvedModule,
        display_name: String,
        version: Option<String>,
        dependencies: Vec<super::DependencyDeclaration>,
        locks: Vec<super::LockedDependency>,
        executables: Vec<ExecutableTarget>,
        tests: Vec<TestTarget>,
    ) -> Self {
        Self {
            id,
            root,
            package_file_path,
            root_module,
            display_name,
            version,
            dependencies,
            locks,
            executables,
            tests,
        }
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn root_module(&self) -> &ResolvedModule {
        &self.root_module
    }

    pub fn package_file_path(&self) -> &Path {
        &self.package_file_path
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
        self.executables.iter().find(|target| target.name() == name)
    }

    pub fn tests(&self) -> &[TestTarget] {
        &self.tests
    }

    pub fn test(&self, name: &str) -> Option<&TestTarget> {
        self.tests.iter().find(|target| target.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(package: &PackageId, path: Option<&str>) -> ModuleId {
        ModuleId::new(
            package.clone(),
            path.map_or(ModuleKey::PackageRoot, |path| {
                ModuleKey::Path(NormalizedModulePath::new(path.to_string()))
            }),
        )
    }

    #[test]
    fn module_identity_contains_only_its_semantic_subtree() {
        let package = PackageId::from_descriptor("package:a");
        let other_package = PackageId::from_descriptor("package:b");
        let root = module(&package, None);
        let text = module(&package, Some("text"));
        let text_search = module(&package, Some("text/search"));
        let textual = module(&package, Some("textual"));

        assert!(root.contains(&text_search));
        assert!(text.contains(&text));
        assert!(text.contains(&text_search));
        assert!(!text.contains(&root));
        assert!(!text.contains(&textual));
        assert!(!text.contains(&module(&other_package, Some("text/search"))));
    }
}
