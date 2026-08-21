use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_model::PackageIdentity;
use nocter_source::{SourceError, SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use crate::{
    DependencySource, ExactDependencyLock, ExactDependencyLockKind, PackageDeclaration,
    PackageDeclarationError, decode_package_declaration,
};

/// One externally resolved package before its authored declaration is loaded and verified.
#[derive(Clone, Debug)]
pub struct ResolvedPackageSpec {
    identity: PackageIdentity,
    root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    locks: BTreeMap<Box<str>, ExactDependencyLock>,
    implicit_dependencies: BTreeMap<Box<str>, PackageIdentity>,
}

impl ResolvedPackageSpec {
    #[must_use]
    pub fn new(identity: PackageIdentity, root: impl Into<PathBuf>) -> Self {
        Self {
            identity,
            root: root.into(),
            dependencies: BTreeMap::new(),
            locks: BTreeMap::new(),
            implicit_dependencies: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_dependency(mut self, alias: impl Into<Box<str>>, package: PackageIdentity) -> Self {
        self.dependencies.insert(alias.into(), package);
        self
    }

    #[must_use]
    pub fn with_lock(mut self, alias: impl Into<Box<str>>, lock: ExactDependencyLock) -> Self {
        self.locks.insert(alias.into(), lock);
        self
    }

    #[must_use]
    pub fn with_standard_dependency(mut self, package: PackageIdentity) -> Self {
        self.implicit_dependencies.insert("std".into(), package);
        self
    }

    #[must_use]
    pub const fn identity(&self) -> &PackageIdentity {
        &self.identity
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// One loaded package whose manifest facts and exact dependency edges have been closed.
#[derive(Debug)]
pub struct ResolvedPackageSnapshot {
    identity: PackageIdentity,
    display_name: Box<str>,
    canonical_root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    locks: BTreeMap<Box<str>, ExactDependencyLock>,
    declaration_path: PathBuf,
    declaration_syntax: usize,
    declaration: Option<PackageDeclaration>,
}

impl ResolvedPackageSnapshot {
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
        &self.canonical_root
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<Box<str>, PackageIdentity> {
        &self.dependencies
    }

    /// Returns the exact effective locks used for remote dependency edges.
    ///
    /// This may include transaction overlays that were validated before their generated source
    /// block was committed.
    #[must_use]
    pub const fn locks(&self) -> &BTreeMap<Box<str>, ExactDependencyLock> {
        &self.locks
    }

    #[must_use]
    pub fn declaration_path(&self) -> &Path {
        &self.declaration_path
    }

    #[must_use]
    pub const fn declaration_syntax(&self) -> usize {
        self.declaration_syntax
    }

    #[must_use]
    pub const fn declaration(&self) -> Option<&PackageDeclaration> {
        self.declaration.as_ref()
    }
}

/// Immutable, syntax-owning exact package graph input for source discovery.
#[derive(Debug)]
pub struct ResolvedPackageGraph {
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    packages: Vec<ResolvedPackageSnapshot>,
}

impl ResolvedPackageGraph {
    /// Loads and validates all exact packages without reopening a manifest in later stages.
    ///
    /// Syntax-invalid manifests remain in the snapshot so the normal diagnostic pipeline can
    /// project them. Every syntax-clean manifest must have dependency edges matching its authored
    /// aliases, remote dependencies must have exact locks, and path edges must select the authored
    /// canonical directory.
    ///
    /// # Errors
    ///
    /// Returns an error for filesystem failures, invalid package data, duplicate identities or
    /// roots, unknown dependency identities, or an inconsistent resolved edge.
    pub fn load(mut specs: Vec<ResolvedPackageSpec>) -> Result<Self, PackageGraphError> {
        specs.sort_unstable_by(|left, right| left.identity.cmp(&right.identity));
        let mut identities = BTreeSet::new();
        for spec in &specs {
            if !identities.insert(spec.identity.clone()) {
                return Err(PackageGraphError::DuplicatePackage(spec.identity.clone()));
            }
        }
        let mut builder = PackageGraphBuilder::new();
        let mut edges = BTreeMap::new();
        for spec in specs {
            let identity = spec.identity.clone();
            builder.load(spec.identity, &spec.root)?;
            if edges
                .insert(
                    identity.clone(),
                    ResolvedPackageEdges {
                        authored: spec.dependencies,
                        locks: spec.locks,
                        implicit: spec.implicit_dependencies,
                    },
                )
                .is_some()
            {
                return Err(PackageGraphError::DuplicatePackage(identity));
            }
        }
        builder.finish(edges)
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub fn syntax_trees(&self) -> &[SyntaxTree] {
        &self.syntax
    }

    #[must_use]
    pub fn packages(&self) -> &[ResolvedPackageSnapshot] {
        &self.packages
    }

    #[must_use]
    pub fn into_parts(self) -> (SourceMap, Vec<SyntaxTree>, Vec<ResolvedPackageSnapshot>) {
        (self.sources, self.syntax, self.packages)
    }
}

pub(crate) struct PackageGraphBuilder {
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    packages: BTreeMap<PackageIdentity, LoadedPackageSnapshot>,
    roots: BTreeMap<PathBuf, PackageIdentity>,
}

impl PackageGraphBuilder {
    pub(crate) fn new() -> Self {
        Self {
            sources: SourceMap::new(),
            syntax: Vec::new(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        }
    }

    pub(crate) fn load(
        &mut self,
        identity: PackageIdentity,
        root: &Path,
    ) -> Result<(), PackageGraphError> {
        let canonical_root = canonical_package_root(root)?;
        self.load_canonical(identity, canonical_root)
    }

    pub(crate) fn load_canonical(
        &mut self,
        identity: PackageIdentity,
        canonical_root: PathBuf,
    ) -> Result<(), PackageGraphError> {
        if self.packages.contains_key(&identity) {
            return Err(PackageGraphError::DuplicatePackage(identity));
        }
        let package = load_package(
            identity.clone(),
            canonical_root,
            &mut self.roots,
            &mut self.sources,
            &mut self.syntax,
        )?;
        self.packages.insert(identity, package);
        Ok(())
    }

    pub(crate) fn declaration(&self, identity: &PackageIdentity) -> Option<&PackageDeclaration> {
        self.packages.get(identity)?.declaration.as_ref()
    }

    pub(crate) fn finish(
        self,
        mut edges: BTreeMap<PackageIdentity, ResolvedPackageEdges>,
    ) -> Result<ResolvedPackageGraph, PackageGraphError> {
        let identities = self.packages.keys().cloned().collect::<BTreeSet<_>>();
        let mut packages = Vec::with_capacity(self.packages.len());
        for (identity, package) in self.packages {
            let resolved = edges
                .remove(&identity)
                .ok_or_else(|| PackageGraphError::UnknownPackage(identity.clone()))?;
            let validated = validate_edges(
                &identity,
                package.declaration.as_ref(),
                resolved.authored,
                resolved.locks,
                resolved.implicit,
                &identities,
            )?;
            packages.push(package.finish(identity, validated.dependencies, validated.locks));
        }
        if let Some(identity) = edges.into_keys().next() {
            return Err(PackageGraphError::UnknownPackage(identity));
        }
        validate_path_roots(&packages)?;
        Ok(ResolvedPackageGraph {
            sources: self.sources,
            syntax: self.syntax,
            packages,
        })
    }
}

pub(crate) struct ResolvedPackageEdges {
    pub(crate) authored: BTreeMap<Box<str>, PackageIdentity>,
    pub(crate) locks: BTreeMap<Box<str>, ExactDependencyLock>,
    pub(crate) implicit: BTreeMap<Box<str>, PackageIdentity>,
}

struct LoadedPackageSnapshot {
    display_name: Box<str>,
    canonical_root: PathBuf,
    declaration_path: PathBuf,
    declaration_syntax: usize,
    declaration: Option<PackageDeclaration>,
}

struct ValidatedPackageEdges {
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    locks: BTreeMap<Box<str>, ExactDependencyLock>,
}

impl LoadedPackageSnapshot {
    fn finish(
        self,
        identity: PackageIdentity,
        dependencies: BTreeMap<Box<str>, PackageIdentity>,
        locks: BTreeMap<Box<str>, ExactDependencyLock>,
    ) -> ResolvedPackageSnapshot {
        ResolvedPackageSnapshot {
            identity,
            display_name: self.display_name,
            canonical_root: self.canonical_root,
            dependencies,
            locks,
            declaration_path: self.declaration_path,
            declaration_syntax: self.declaration_syntax,
            declaration: self.declaration,
        }
    }
}

fn load_package(
    identity: PackageIdentity,
    canonical_root: PathBuf,
    roots: &mut BTreeMap<PathBuf, PackageIdentity>,
    sources: &mut SourceMap,
    syntax: &mut Vec<SyntaxTree>,
) -> Result<LoadedPackageSnapshot, PackageGraphError> {
    if !canonical_root.is_dir() {
        return Err(PackageGraphError::InvalidPackageRoot {
            package: identity,
            path: canonical_root,
        });
    }
    if let Some(first) = roots.insert(canonical_root.clone(), identity.clone()) {
        return Err(PackageGraphError::DuplicateCanonicalRoot {
            first,
            second: identity,
            path: canonical_root,
        });
    }
    let declaration_path = canonical_root.join("nocter.nct");
    if !regular_file(&declaration_path)? {
        return Err(PackageGraphError::MissingPackageFile {
            package: identity,
            path: declaration_path,
        });
    }
    let declaration_path = canonicalize("canonicalize package file", &declaration_path)?;
    if !declaration_path.starts_with(&canonical_root) {
        return Err(PackageGraphError::InvalidPackageRoot {
            package: identity,
            path: declaration_path,
        });
    }
    let bytes = fs::read(&declaration_path).map_err(|error| PackageGraphError::Filesystem {
        operation: "read",
        path: declaration_path.clone(),
        error,
    })?;
    let canonical_name = canonical_text(&declaration_path)?;
    let source_id = sources
        .add_bytes(SourceName::new(canonical_name.as_ref()), &bytes)
        .map_err(|error| PackageGraphError::Source {
            path: declaration_path.clone(),
            error,
        })?;
    let tree = parse(
        sources
            .get(source_id)
            .expect("new package source remains in the source map"),
        ParseGoal::PackageFile,
    );
    let declaration = if tree.has_errors() {
        None
    } else {
        let source = sources
            .get(source_id)
            .expect("parsed package source remains in the source map");
        Some(decode_package_declaration(source, &tree).map_err(PackageGraphError::Declaration)?)
    };
    let display_name = declaration
        .as_ref()
        .and_then(PackageDeclaration::name)
        .map_or_else(
            || directory_name(&canonical_root),
            |name| Ok(Box::<str>::from(name.value())),
        )?;
    let declaration_syntax = syntax.len();
    syntax.push(tree);
    Ok(LoadedPackageSnapshot {
        display_name,
        canonical_root,
        declaration_path,
        declaration_syntax,
        declaration,
    })
}

pub(crate) fn canonical_package_root(path: &Path) -> Result<PathBuf, PackageGraphError> {
    canonicalize("canonicalize package root", path)
}

fn validate_edges(
    package: &PackageIdentity,
    declaration: Option<&PackageDeclaration>,
    authored_edges: BTreeMap<Box<str>, PackageIdentity>,
    mut resolved_locks: BTreeMap<Box<str>, ExactDependencyLock>,
    implicit_edges: BTreeMap<Box<str>, PackageIdentity>,
    identities: &BTreeSet<PackageIdentity>,
) -> Result<ValidatedPackageEdges, PackageGraphError> {
    for dependency in authored_edges.values().chain(implicit_edges.values()) {
        if !identities.contains(dependency) {
            return Err(PackageGraphError::UnknownPackage(dependency.clone()));
        }
    }
    if let Some(declaration) = declaration {
        let declared = declaration.dependencies().keys().collect::<BTreeSet<_>>();
        let resolved = authored_edges.keys().collect::<BTreeSet<_>>();
        if declared != resolved {
            let alias = declared
                .symmetric_difference(&resolved)
                .next()
                .expect("unequal alias sets have a difference");
            return Err(PackageGraphError::DependencyEdgeMismatch {
                package: package.clone(),
                alias: (*alias).clone(),
            });
        }
        for (alias, dependency) in declaration.dependencies() {
            match dependency.source() {
                DependencySource::Path { .. } => {
                    if resolved_locks.remove(alias).is_some() {
                        return Err(PackageGraphError::UnexpectedResolvedLock {
                            package: package.clone(),
                            alias: alias.clone(),
                        });
                    }
                }
                DependencySource::Git { .. } | DependencySource::Archive { .. } => {
                    let authored = declaration
                        .locks()
                        .get(alias)
                        .map(crate::DependencyLock::exact);
                    let selected = resolved_locks
                        .get(alias)
                        .cloned()
                        .or_else(|| authored.clone());
                    let Some(selected) = selected else {
                        return Err(PackageGraphError::MissingLock {
                            package: package.clone(),
                            alias: alias.clone(),
                        });
                    };
                    if authored
                        .as_ref()
                        .is_some_and(|authored| authored != &selected)
                    {
                        return Err(PackageGraphError::LockSelectionMismatch {
                            package: package.clone(),
                            alias: alias.clone(),
                        });
                    }
                    let expected = match dependency.source() {
                        DependencySource::Git { .. } => ExactDependencyLockKind::Git,
                        DependencySource::Archive { .. } => ExactDependencyLockKind::Sha256,
                        DependencySource::Path { .. } => unreachable!(),
                    };
                    if selected.kind() != expected {
                        return Err(PackageGraphError::LockKindMismatch {
                            package: package.clone(),
                            alias: alias.clone(),
                        });
                    }
                    resolved_locks.insert(alias.clone(), selected);
                }
            }
        }
        if let Some(alias) = resolved_locks
            .keys()
            .find(|alias| !declaration.dependencies().contains_key(*alias))
        {
            return Err(PackageGraphError::UnexpectedResolvedLock {
                package: package.clone(),
                alias: alias.clone(),
            });
        }
    } else if let Some(alias) = resolved_locks.keys().next() {
        return Err(PackageGraphError::UnexpectedResolvedLock {
            package: package.clone(),
            alias: alias.clone(),
        });
    }
    let mut dependencies = authored_edges;
    for (alias, target) in implicit_edges {
        if alias.as_ref() != "std" || dependencies.insert(alias.clone(), target).is_some() {
            return Err(PackageGraphError::InvalidImplicitDependency {
                package: package.clone(),
                alias,
            });
        }
    }
    Ok(ValidatedPackageEdges {
        dependencies,
        locks: resolved_locks,
    })
}

fn validate_path_roots(packages: &[ResolvedPackageSnapshot]) -> Result<(), PackageGraphError> {
    let roots = packages
        .iter()
        .map(|package| (package.identity(), package.root()))
        .collect::<BTreeMap<_, _>>();
    for package in packages {
        let Some(declaration) = package.declaration() else {
            continue;
        };
        for (alias, dependency) in declaration.dependencies() {
            let DependencySource::Path { path } = dependency.source() else {
                continue;
            };
            let expected = canonicalize(
                "canonicalize path dependency",
                &package.root().join(path.value()),
            )?;
            let target = package
                .dependencies()
                .get(alias)
                .expect("validated authored alias has a resolved edge");
            let actual = roots
                .get(target)
                .expect("validated dependency identity has a loaded root");
            if expected != **actual {
                return Err(PackageGraphError::InvalidPathDependency {
                    package: package.identity().clone(),
                    alias: alias.clone(),
                    path: expected,
                });
            }
        }
    }
    Ok(())
}

fn canonicalize(operation: &'static str, path: &Path) -> Result<PathBuf, PackageGraphError> {
    fs::canonicalize(path).map_err(|error| PackageGraphError::Filesystem {
        operation,
        path: path.into(),
        error,
    })
}

fn regular_file(path: &Path) -> Result<bool, PackageGraphError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PackageGraphError::Filesystem {
            operation: "inspect",
            path: path.into(),
            error,
        }),
    }
}

fn canonical_text(path: &Path) -> Result<Box<str>, PackageGraphError> {
    path.to_str()
        .map(Into::into)
        .ok_or_else(|| PackageGraphError::NonUnicodeCanonicalPath(path.into()))
}

fn directory_name(path: &Path) -> Result<Box<str>, PackageGraphError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(Into::into)
        .ok_or_else(|| PackageGraphError::NonUnicodeCanonicalPath(path.into()))
}

#[derive(Debug)]
pub enum PackageGraphError {
    DuplicatePackage(PackageIdentity),
    UnknownPackage(PackageIdentity),
    InvalidPackageRoot {
        package: PackageIdentity,
        path: PathBuf,
    },
    MissingPackageFile {
        package: PackageIdentity,
        path: PathBuf,
    },
    DuplicateCanonicalRoot {
        first: PackageIdentity,
        second: PackageIdentity,
        path: PathBuf,
    },
    DependencyEdgeMismatch {
        package: PackageIdentity,
        alias: Box<str>,
    },
    MissingLock {
        package: PackageIdentity,
        alias: Box<str>,
    },
    UnexpectedResolvedLock {
        package: PackageIdentity,
        alias: Box<str>,
    },
    LockSelectionMismatch {
        package: PackageIdentity,
        alias: Box<str>,
    },
    LockKindMismatch {
        package: PackageIdentity,
        alias: Box<str>,
    },
    InvalidPathDependency {
        package: PackageIdentity,
        alias: Box<str>,
        path: PathBuf,
    },
    InvalidImplicitDependency {
        package: PackageIdentity,
        alias: Box<str>,
    },
    NonUnicodeCanonicalPath(PathBuf),
    Declaration(PackageDeclarationError),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    Source {
        path: PathBuf,
        error: SourceError,
    },
}

impl fmt::Display for PackageGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePackage(package) => {
                write!(formatter, "duplicate resolved package {}", package.as_str())
            }
            Self::UnknownPackage(package) => {
                write!(formatter, "unknown resolved package {}", package.as_str())
            }
            Self::InvalidPackageRoot { package, path } => write!(
                formatter,
                "package {} has invalid root {}",
                package.as_str(),
                path.display()
            ),
            Self::MissingPackageFile { package, path } => write!(
                formatter,
                "package {} has no package file at {}",
                package.as_str(),
                path.display()
            ),
            Self::DuplicateCanonicalRoot {
                first,
                second,
                path,
            } => write!(
                formatter,
                "packages {} and {} share canonical root {}",
                first.as_str(),
                second.as_str(),
                path.display()
            ),
            Self::DependencyEdgeMismatch { package, alias } => write!(
                formatter,
                "package {} has no exact resolved edge for dependency {alias}",
                package.as_str()
            ),
            Self::MissingLock { package, alias } => write!(
                formatter,
                "package {} has no exact lock for dependency {alias}",
                package.as_str()
            ),
            Self::UnexpectedResolvedLock { package, alias } => write!(
                formatter,
                "package {} has an exact lock for non-remote dependency {alias}",
                package.as_str()
            ),
            Self::LockSelectionMismatch { package, alias } => write!(
                formatter,
                "package {} dependency {alias} has conflicting authored and resolved locks",
                package.as_str()
            ),
            Self::LockKindMismatch { package, alias } => write!(
                formatter,
                "package {} dependency {alias} has an incompatible exact lock kind",
                package.as_str()
            ),
            Self::InvalidPathDependency {
                package,
                alias,
                path,
            } => write!(
                formatter,
                "package {} path dependency {alias} has invalid root {}",
                package.as_str(),
                path.display()
            ),
            Self::InvalidImplicitDependency { package, alias } => write!(
                formatter,
                "package {} has invalid implicit dependency {alias}",
                package.as_str()
            ),
            Self::NonUnicodeCanonicalPath(path) => {
                write!(
                    formatter,
                    "canonical path is not Unicode: {}",
                    path.display()
                )
            }
            Self::Declaration(error) => error.fmt(formatter),
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
            Self::Source { path, error } => {
                write!(formatter, "cannot ingest {}: {error:?}", path.display())
            }
        }
    }
}

impl std::error::Error for PackageGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declaration(error) => Some(error),
            Self::Filesystem { error, .. } => Some(error),
            Self::DuplicatePackage(_)
            | Self::UnknownPackage(_)
            | Self::InvalidPackageRoot { .. }
            | Self::MissingPackageFile { .. }
            | Self::DuplicateCanonicalRoot { .. }
            | Self::DependencyEdgeMismatch { .. }
            | Self::MissingLock { .. }
            | Self::UnexpectedResolvedLock { .. }
            | Self::LockSelectionMismatch { .. }
            | Self::LockKindMismatch { .. }
            | Self::InvalidPathDependency { .. }
            | Self::InvalidImplicitDependency { .. }
            | Self::NonUnicodeCanonicalPath(_)
            | Self::Source { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-package-graph-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn source(&self, relative: &str, text: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn identity(value: &str) -> PackageIdentity {
        PackageIdentity::new(value)
    }

    #[test]
    fn loads_manifest_sources_and_closes_exact_path_edges() {
        let tree = TempTree::new();
        tree.source(
            "app/nocter.nct",
            "#name: \"application\"\n#dependencies: { util: { path: \"../util\", }, }\n#executable: { name: \"app\", }\n",
        );
        tree.source("util/nocter.nct", "#name: \"utility\"\n");
        let graph = ResolvedPackageGraph::load(vec![
            ResolvedPackageSpec::new(identity("root"), tree.0.join("app"))
                .with_dependency("util", identity("util")),
            ResolvedPackageSpec::new(identity("util"), tree.0.join("util")),
        ])
        .unwrap();

        assert_eq!(graph.sources().len(), 2);
        assert_eq!(graph.syntax_trees().len(), 2);
        assert_eq!(graph.packages()[0].display_name(), "application");
        assert_eq!(
            graph.packages()[0].declaration().unwrap().targets().len(),
            1
        );
        assert_eq!(
            graph.packages()[0].dependencies().get("util"),
            Some(&identity("util"))
        );
    }

    #[test]
    fn rejects_edges_that_disagree_with_authored_dependencies_and_locks() {
        let tree = TempTree::new();
        tree.source(
            "app/nocter.nct",
            "#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n",
        );
        tree.source("remote/nocter.nct", "#name: \"remote\"\n");
        let missing_lock = ResolvedPackageGraph::load(vec![
            ResolvedPackageSpec::new(identity("app"), tree.0.join("app"))
                .with_dependency("remote", identity("remote")),
            ResolvedPackageSpec::new(identity("remote"), tree.0.join("remote")),
        ])
        .unwrap_err();
        assert!(matches!(
            missing_lock,
            PackageGraphError::MissingLock { .. }
        ));

        tree.source("empty/nocter.nct", "#name: \"empty\"\n");
        let extra_edge = ResolvedPackageGraph::load(vec![
            ResolvedPackageSpec::new(identity("empty"), tree.0.join("empty"))
                .with_dependency("remote", identity("remote")),
            ResolvedPackageSpec::new(identity("remote"), tree.0.join("remote")),
        ])
        .unwrap_err();
        assert!(matches!(
            extra_edge,
            PackageGraphError::DependencyEdgeMismatch { .. }
        ));
    }

    #[test]
    fn validates_and_retains_a_provisional_exact_lock() {
        let tree = TempTree::new();
        tree.source(
            "app/nocter.nct",
            "#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n",
        );
        tree.source("remote/nocter.nct", "#name: \"remote\"\n");
        let lock = ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap();

        let graph = ResolvedPackageGraph::load(vec![
            ResolvedPackageSpec::new(identity("app"), tree.0.join("app"))
                .with_dependency("remote", identity("remote"))
                .with_lock("remote", lock.clone()),
            ResolvedPackageSpec::new(identity("remote"), tree.0.join("remote")),
        ])
        .unwrap();

        assert_eq!(graph.packages()[0].locks().get("remote"), Some(&lock));
        assert!(
            graph.packages()[0]
                .declaration()
                .unwrap()
                .locks()
                .is_empty()
        );
    }

    #[test]
    fn rejects_a_provisional_lock_that_changes_an_authored_selection() {
        let tree = TempTree::new();
        tree.source(
            "app/nocter.nct",
            "#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n#lock: { format: 1, dependencies: { remote: \"git:7db21c1000000000000000000000000000000000\", }, }\n",
        );
        tree.source("remote/nocter.nct", "#name: \"remote\"\n");
        let replacement =
            ExactDependencyLock::git("8db21c1000000000000000000000000000000000").unwrap();

        let error = ResolvedPackageGraph::load(vec![
            ResolvedPackageSpec::new(identity("app"), tree.0.join("app"))
                .with_dependency("remote", identity("remote"))
                .with_lock("remote", replacement),
            ResolvedPackageSpec::new(identity("remote"), tree.0.join("remote")),
        ])
        .unwrap_err();

        assert!(matches!(
            error,
            PackageGraphError::LockSelectionMismatch { .. }
        ));
    }

    #[test]
    fn retains_syntax_invalid_manifests_for_normal_diagnostic_projection() {
        let tree = TempTree::new();
        tree.source("app/nocter.nct", "#name: { nested: \"app\"\n");
        let graph = ResolvedPackageGraph::load(vec![ResolvedPackageSpec::new(
            identity("app"),
            tree.0.join("app"),
        )])
        .unwrap();

        assert!(graph.syntax_trees()[0].has_errors());
        assert!(graph.packages()[0].declaration().is_none());
    }
}
