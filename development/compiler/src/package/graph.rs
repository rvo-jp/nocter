use super::fetch::{archive_digest, fetch, resolve_git_revision};
use super::loader::load_package_with_id_and_overlay;
use super::lockfile::write_generated_lock;
use super::store::PackageStore;
use super::{
    DependencyDeclaration, DependencyLock, DependencySource, LockedDependency, PackageId,
    SourcePackage,
};
use crate::diagnostics::Diagnostic;
use crate::package::standard_library::{
    STANDARD_LIBRARY_ALIAS, StandardLibrarySelection, validation_errors,
};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

const PACKAGE_GRAPH_ERROR: &str = "E0801";

#[derive(Debug, Clone, Copy, Default)]
pub struct PackageGraphOptions {
    pub locked: bool,
    pub offline: bool,
}

#[derive(Debug, Clone)]
pub struct PackageGraph {
    root: PackageId,
    standard_library: PackageId,
    packages: BTreeMap<PackageId, SourcePackage>,
    namespaces: BTreeMap<(PackageId, String), PackageId>,
}

impl PackageGraph {
    pub fn root_package(&self) -> &SourcePackage {
        self.packages
            .get(&self.root)
            .expect("package graph root must exist")
    }

    pub fn packages(&self) -> impl Iterator<Item = &SourcePackage> {
        self.packages.values()
    }

    pub fn standard_library(&self) -> &SourcePackage {
        self.packages
            .get(&self.standard_library)
            .expect("package graph standard library must exist")
    }

    pub fn is_standard_library_package(&self, id: &PackageId) -> bool {
        &self.standard_library == id
    }

    pub fn package_containing(&self, source: &Path) -> Option<&SourcePackage> {
        self.packages
            .values()
            .filter(|package| source.starts_with(package.root()))
            .max_by_key(|package| package.root().components().count())
    }

    pub fn dependency(&self, owner: &PackageId, name: &str) -> Option<&SourcePackage> {
        let id = self.namespaces.get(&(owner.clone(), name.to_string()))?;
        self.packages.get(id)
    }

    pub fn dependency_names<'a>(&'a self, owner: &'a PackageId) -> impl Iterator<Item = &'a str> {
        self.namespaces
            .keys()
            .filter(move |(package, _)| package == owner)
            .map(|(_, name)| name.as_str())
    }

    pub(crate) fn dependency_name(
        &self,
        owner: &PackageId,
        dependency: &PackageId,
    ) -> Option<&str> {
        self.namespaces
            .iter()
            .find(|((candidate_owner, _), candidate_dependency)| {
                candidate_owner == owner && *candidate_dependency == dependency
            })
            .map(|((_, name), _)| name.as_str())
    }

    pub fn is_package_file(&self, path: &Path) -> bool {
        self.packages
            .values()
            .any(|package| package.package_file_path() == path)
    }
}

pub struct PackageGraphLoad {
    pub graph: Option<PackageGraph>,
    pub diagnostics: Vec<Diagnostic>,
    pub lock_changed: bool,
    pub package_files: HashSet<PathBuf>,
}

pub fn load_package_graph(root: &Path, options: PackageGraphOptions) -> PackageGraphLoad {
    load_package_graph_impl(
        root,
        options,
        &super::PackageSourceOverlay::default(),
        true,
        StandardLibrarySelection::active(),
    )
}

/// Resolves the exact graph without rewriting the source-owned package lock.
pub fn inspect_package_graph(root: &Path, options: PackageGraphOptions) -> PackageGraphLoad {
    load_package_graph_impl(
        root,
        options,
        &super::PackageSourceOverlay::default(),
        false,
        StandardLibrarySelection::active(),
    )
}

pub(crate) fn load_locked_offline_package_graph_with_overlay(
    root: &Path,
    overlay: &super::PackageSourceOverlay,
) -> PackageGraphLoad {
    load_package_graph_impl(
        root,
        PackageGraphOptions {
            locked: true,
            offline: true,
        },
        overlay,
        false,
        StandardLibrarySelection::active(),
    )
}

fn load_package_graph_impl(
    root: &Path,
    options: PackageGraphOptions,
    overlay: &super::PackageSourceOverlay,
    write_lock: bool,
    standard_library: Result<StandardLibrarySelection, String>,
) -> PackageGraphLoad {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let root_id = standard_library.as_ref().ok().and_then(|selection| {
        let standard_root = selection
            .root()
            .canonicalize()
            .unwrap_or_else(|_| selection.root().to_path_buf());
        (root == standard_root).then(|| selection.package_id())
    });
    let store = PackageStore::new(&root);
    let mut builder = GraphBuilder {
        options,
        overlay,
        store,
        packages: BTreeMap::new(),
        namespaces: BTreeMap::new(),
        visiting: HashSet::new(),
        package_files: HashSet::new(),
        diagnostics: Vec::new(),
        pending_lock_writes: Vec::new(),
        lock_changed: false,
        standard_library,
    };
    let root_id = builder.visit(&root, root_id, true);
    if !builder.diagnostics.is_empty() {
        return PackageGraphLoad {
            graph: None,
            diagnostics: builder.diagnostics,
            lock_changed: builder.lock_changed,
            package_files: builder.package_files,
        };
    }
    let standard_library = root_id
        .as_ref()
        .and_then(|_| builder.attach_standard_library());
    if !builder.diagnostics.is_empty() {
        return PackageGraphLoad {
            graph: None,
            diagnostics: builder.diagnostics,
            lock_changed: builder.lock_changed,
            package_files: builder.package_files,
        };
    }
    let standard_library = standard_library
        .expect("successful package graph construction must attach the toolchain standard library");
    let package_ids = builder.packages.keys().cloned().collect::<Vec<_>>();
    for package in package_ids {
        builder.namespaces.insert(
            (package, STANDARD_LIBRARY_ALIAS.to_string()),
            standard_library.clone(),
        );
    }
    if write_lock {
        builder.commit_locks();
    }
    if !builder.diagnostics.is_empty() {
        return PackageGraphLoad {
            graph: None,
            diagnostics: builder.diagnostics,
            lock_changed: builder.lock_changed,
            package_files: builder.package_files,
        };
    }
    PackageGraphLoad {
        graph: root_id.map(|root| PackageGraph {
            root,
            standard_library,
            packages: builder.packages,
            namespaces: builder.namespaces,
        }),
        diagnostics: Vec::new(),
        lock_changed: builder.lock_changed,
        package_files: builder.package_files,
    }
}

struct GraphBuilder<'a> {
    options: PackageGraphOptions,
    overlay: &'a super::PackageSourceOverlay,
    store: PackageStore,
    packages: BTreeMap<PackageId, SourcePackage>,
    namespaces: BTreeMap<(PackageId, String), PackageId>,
    visiting: HashSet<PathBuf>,
    package_files: HashSet<PathBuf>,
    diagnostics: Vec<Diagnostic>,
    pending_lock_writes: Vec<(PathBuf, Vec<LockedDependency>)>,
    lock_changed: bool,
    standard_library: Result<StandardLibrarySelection, String>,
}

impl GraphBuilder<'_> {
    fn attach_standard_library(&mut self) -> Option<PackageId> {
        let selection = match self.standard_library.clone() {
            Ok(selection) => selection,
            Err(error) => {
                self.error(error);
                return None;
            }
        };
        let root = selection.root().to_path_buf();
        let package_file = root.join("nocter.nct");
        let root_source = root.join("index.nct");
        if !package_file.is_file() {
            self.error(format!(
                "toolchain standard-library package is missing `{}`",
                package_file.display()
            ));
        }
        if !root_source.is_file() {
            self.error(format!(
                "toolchain standard-library package is missing `{}`",
                root_source.display()
            ));
        }
        if !self.diagnostics.is_empty() {
            return None;
        }
        let root = root.canonicalize().unwrap_or(root);
        let package_file = root.join("nocter.nct");
        self.package_files.insert(package_file);
        if let Some((id, package)) = self
            .packages
            .iter()
            .find(|(_, package)| package.root() == root)
            .map(|(id, package)| (id.clone(), package.clone()))
        {
            return self
                .validate_standard_library_package(&package, Some(selection.expected_version()))
                .then_some(id);
        }
        let probe = load_package_with_id_and_overlay(&root, None, Some(self.overlay));
        if !probe.diagnostics.is_empty() {
            self.diagnostics.extend(probe.diagnostics);
            return None;
        }
        let probe = probe
            .package
            .expect("successful standard-library package load");
        if !self.validate_standard_library_package(&probe, Some(selection.expected_version())) {
            return None;
        }
        let id = selection.package_id();
        let load = load_package_with_id_and_overlay(&root, Some(id.clone()), Some(self.overlay));
        if !load.diagnostics.is_empty() {
            self.diagnostics.extend(load.diagnostics);
            return None;
        }
        self.packages.insert(
            id.clone(),
            load.package
                .expect("successful standard-library package load with fixed identity"),
        );
        Some(id)
    }

    fn validate_standard_library_package(
        &mut self,
        package: &SourcePackage,
        expected_version: Option<&str>,
    ) -> bool {
        let errors = validation_errors(package, expected_version);
        let valid = errors.is_empty();
        for error in errors {
            self.error(error);
        }
        valid
    }

    fn visit(
        &mut self,
        root: &Path,
        expected_id: Option<PackageId>,
        may_generate_lock: bool,
    ) -> Option<PackageId> {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        self.package_files.insert(canonical.join("nocter.nct"));
        if !self.visiting.insert(canonical.clone()) {
            self.error(format!(
                "dependency cycle reaches package `{}`",
                canonical.display()
            ));
            return None;
        }
        let load = load_package_with_id_and_overlay(&canonical, expected_id, Some(self.overlay));
        if !load.diagnostics.is_empty() {
            self.diagnostics.extend(load.diagnostics);
            self.visiting.remove(&canonical);
            return None;
        }
        let mut package = load.package.expect("successful package load");
        let id = package.id().clone();
        if self.packages.contains_key(&id) {
            self.visiting.remove(&canonical);
            return Some(id);
        }
        let mut locks = package.locks().to_vec();
        let mut package_lock_changed = false;
        for dependency in package.dependencies() {
            if dependency.name() == STANDARD_LIBRARY_ALIAS {
                self.error("dependency name `std` is reserved for the toolchain standard library");
                continue;
            }
            let resolution = match self.resolve_dependency(&package, dependency, may_generate_lock)
            {
                Some(resolution) => resolution,
                None => continue,
            };
            if let Some(new_lock) = resolution.generated_lock {
                locks.retain(|lock| lock.name() != dependency.name());
                locks.push(new_lock);
                self.lock_changed = true;
                package_lock_changed = true;
            }
            let Some(child_id) = self.visit(&resolution.root, resolution.id, resolution.mutable)
            else {
                continue;
            };
            self.namespaces
                .insert((id.clone(), dependency.name().to_string()), child_id);
        }
        if may_generate_lock && package_lock_changed {
            package.replace_locks(locks.clone());
            self.pending_lock_writes
                .push((package.package_file_path().to_path_buf(), locks));
        }
        self.packages.insert(id.clone(), package);
        self.visiting.remove(&canonical);
        Some(id)
    }

    fn resolve_dependency(
        &mut self,
        package: &SourcePackage,
        declaration: &DependencyDeclaration,
        may_generate_lock: bool,
    ) -> Option<ResolvedDependency> {
        if let DependencySource::Path { path } = declaration.source() {
            if package.lock(declaration.name()).is_some() {
                self.error(format!(
                    "path dependency `{}` must not have a lock entry",
                    declaration.name()
                ));
                return None;
            }
            let root = package.root().join(path);
            let canonical = match root.canonicalize() {
                Ok(root) => root,
                Err(error) => {
                    self.error(format!(
                        "path dependency `{}` cannot be resolved at `{}`: {error}",
                        declaration.name(),
                        root.display()
                    ));
                    return None;
                }
            };
            return Some(ResolvedDependency {
                root: canonical,
                id: None,
                mutable: true,
                generated_lock: None,
            });
        }
        let mut generated_lock = None;
        let resolution = if let Some(lock) = package.lock(declaration.name()) {
            lock.clone()
        } else {
            if self.options.locked || self.options.offline || !may_generate_lock {
                self.error(format!(
                    "dependency `{}` requires a generated lock; run `nocter fetch` in its package",
                    declaration.name()
                ));
                return None;
            }
            let lock = match declaration.source() {
                DependencySource::Git { url, revision } => {
                    resolve_git_revision(url, revision).map(DependencyLock::GitCommit)
                }
                DependencySource::Archive { url } => {
                    archive_digest(url, self.store.local_root()).map(DependencyLock::ArchiveSha256)
                }
                DependencySource::Path { .. } => unreachable!(),
            };
            let lock = match lock {
                Ok(lock) => lock,
                Err(error) => {
                    self.error(format!(
                        "failed to resolve dependency `{}`: {error}",
                        declaration.name()
                    ));
                    return None;
                }
            };
            generated_lock = Some(LockedDependency::new(
                declaration.name().to_string(),
                declaration.span(),
                lock.clone(),
            ));
            lock
        };
        if !lock_matches_source(declaration.source(), &resolution) {
            self.error(format!(
                "dependency `{}` source and lock kinds do not match",
                declaration.name()
            ));
            return None;
        }
        let descriptor = declaration
            .source()
            .identity_descriptor(package.root(), Some(&resolution))
            .expect("locked dependency has an identity descriptor");
        let id = PackageId::from_descriptor(&descriptor);
        let root = if let Some(root) = self.store.find(&id) {
            root
        } else {
            if self.options.offline {
                self.error(format!(
                    "dependency `{}` is not cached for offline use",
                    declaration.name()
                ));
                return None;
            }
            match fetch(
                declaration.source(),
                &resolution,
                &id,
                self.store.local_root(),
            ) {
                Ok(result) => {
                    debug_assert_eq!(result.resolution, resolution);
                    result.root
                }
                Err(error) => {
                    self.error(format!(
                        "failed to fetch dependency `{}`: {error}",
                        declaration.name()
                    ));
                    return None;
                }
            }
        };
        Some(ResolvedDependency {
            root,
            id: Some(id),
            mutable: false,
            generated_lock,
        })
    }

    fn error(&mut self, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(PACKAGE_GRAPH_ERROR, message));
    }

    fn commit_locks(&mut self) {
        for (path, locks) in std::mem::take(&mut self.pending_lock_writes) {
            if let Err(error) = write_generated_lock(&path, &locks) {
                self.error(error);
                break;
            }
        }
    }
}

struct ResolvedDependency {
    root: PathBuf,
    id: Option<PackageId>,
    mutable: bool,
    generated_lock: Option<LockedDependency>,
}

fn lock_matches_source(source: &DependencySource, lock: &DependencyLock) -> bool {
    matches!(
        (source, lock),
        (DependencySource::Git { .. }, DependencyLock::GitCommit(_))
            | (
                DependencySource::Archive { .. },
                DependencyLock::ArchiveSha256(_)
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn attaches_the_toolchain_standard_library_as_an_implicit_dependency() {
        let sandbox = temp_directory("implicit-std");
        let package = sandbox.join("app");
        let standard_library = sandbox.join("home/std");
        write_package(&package, "#name: \"app\"\n");
        write_package(&standard_library, "#name: \"std\"\n#version: \"0.9.0\"\n");

        let load = load_package_graph_impl(
            &package,
            PackageGraphOptions::default(),
            &super::super::PackageSourceOverlay::default(),
            false,
            Ok(StandardLibrarySelection::new(
                standard_library.clone(),
                "0.9.0".to_string(),
            )),
        );
        assert!(load.diagnostics.is_empty(), "{:?}", load.diagnostics);
        let graph = load.graph.unwrap();
        let root = graph.root_package();
        let std = graph.standard_library();
        assert_eq!(std.root(), standard_library.canonicalize().unwrap());
        assert_eq!(graph.dependency(root.id(), "std"), Some(std));
        assert_eq!(graph.packages().count(), 2);

        let mismatch = load_package_graph_impl(
            &package,
            PackageGraphOptions::default(),
            &super::super::PackageSourceOverlay::default(),
            false,
            Ok(StandardLibrarySelection::new(
                standard_library.clone(),
                "0.10.0".to_string(),
            )),
        );
        assert!(mismatch.graph.is_none());
        assert!(mismatch.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not match Nocter home version")
        }));
        fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    fn the_standard_library_graph_uses_one_package_identity_for_itself() {
        let sandbox = temp_directory("std-self-identity");
        let standard_library = sandbox.join("home/std");
        write_package(&standard_library, "#name: \"std\"\n#version: \"0.9.0\"\n");

        let load = load_package_graph_impl(
            &standard_library,
            PackageGraphOptions::default(),
            &super::super::PackageSourceOverlay::default(),
            false,
            Ok(StandardLibrarySelection::new(
                standard_library.clone(),
                "0.9.0".to_string(),
            )),
        );
        assert!(load.diagnostics.is_empty(), "{:?}", load.diagnostics);
        let graph = load.graph.unwrap();
        let root = graph.root_package();
        assert!(graph.is_standard_library_package(root.id()));
        assert_eq!(
            root.id(),
            &PackageId::standard_library(&standard_library, Some("0.9.0"))
        );
        assert_eq!(graph.dependency(root.id(), "std"), Some(root));
        assert_eq!(graph.packages().count(), 1);
        fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    fn rejects_an_explicit_dependency_named_std() {
        let sandbox = temp_directory("reserved-std-dependency");
        let package = sandbox.join("app");
        let fake = package.join("fake");
        let standard_library = sandbox.join("home/std");
        write_package(
            &package,
            "#name: \"app\"\n#dependencies: { std: { path: \"./fake\" } }\n",
        );
        write_package(&fake, "#name: \"fake\"\n");
        write_package(&standard_library, "#name: \"std\"\n#version: \"0.9.0\"\n");

        let load = load_package_graph_impl(
            &package,
            PackageGraphOptions::default(),
            &super::super::PackageSourceOverlay::default(),
            false,
            Ok(StandardLibrarySelection::new(
                standard_library,
                "0.9.0".to_string(),
            )),
        );
        assert!(load.graph.is_none());
        assert!(load.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("dependency name `std` is reserved")
        }));
        fs::remove_dir_all(sandbox).unwrap();
    }

    #[test]
    fn rejects_a_graph_when_the_selected_standard_package_is_missing() {
        let sandbox = temp_directory("missing-implicit-std");
        let package = sandbox.join("app");
        let standard_library = sandbox.join("home/std");
        write_package(&package, "#name: \"app\"\n");

        let load = load_package_graph_impl(
            &package,
            PackageGraphOptions::default(),
            &super::super::PackageSourceOverlay::default(),
            false,
            Ok(StandardLibrarySelection::new(
                standard_library,
                "0.9.0".to_string(),
            )),
        );

        assert!(load.graph.is_none());
        assert_eq!(
            load.diagnostics
                .iter()
                .filter(|diagnostic| diagnostic
                    .message
                    .contains("standard-library package is missing"))
                .count(),
            2
        );
        fs::remove_dir_all(sandbox).unwrap();
    }

    fn write_package(root: &Path, package_source: &str) {
        crate::test_files::write(root.join("nocter.nct"), package_source).unwrap();
        crate::test_files::write(root.join("index.nct"), "").unwrap();
    }

    fn temp_directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "nocter-package-graph-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
