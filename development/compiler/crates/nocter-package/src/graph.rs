use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_filesystem::SourceOverlay;
use nocter_model::PackageIdentity;
use nocter_source::{SourceError, SourceMap, SourceName};
use nocter_syntax::{DirectSourceSyntax, SourceSyntaxProvider, SyntaxTree};

use crate::{
    DependencySource, ExactDependencyLock, ExactDependencyLockKind, PackageDeclaration,
    PackageDeclarationError, PackageLockSourceError, PackageLockSourceUpdate, PackageRootCatalog,
    PackageRootCatalogBuilder, PackageRootProbeError, decode_package_declaration,
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

/// One loaded package whose root-source facts and exact dependency edges have been closed.
#[derive(Clone, Debug)]
pub struct ResolvedPackageSnapshot {
    identity: PackageIdentity,
    display_name: Box<str>,
    canonical_root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    locks: BTreeMap<Box<str>, ExactDependencyLock>,
    declaration_path: PathBuf,
    root_source_bytes: Box<[u8]>,
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
    /// This may see transaction overlays that were validated before their generated source
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
#[derive(Clone, Debug)]
pub struct ResolvedPackageGraph {
    package_roots: PackageRootCatalog,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    packages: Vec<ResolvedPackageSnapshot>,
}

/// Reached package source and syntax retained even when exact graph resolution fails.
#[derive(Clone, Debug)]
pub struct PackageSourceSnapshot {
    package_roots: PackageRootCatalog,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
}

impl PackageSourceSnapshot {
    pub(crate) fn from_root_catalog(package_roots: PackageRootCatalog) -> Self {
        Self {
            package_roots,
            sources: SourceMap::new(),
            syntax: Vec::new(),
        }
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        self.package_roots.source_overlay()
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub fn syntax_trees(&self) -> &[SyntaxTree] {
        &self.syntax
    }
}

impl ResolvedPackageGraph {
    /// Loads and validates all exact packages without reopening a root source in later stages.
    ///
    /// Syntax-invalid root sources remain in the snapshot so the normal diagnostic pipeline can
    /// project them. Every syntax-clean root source must have dependency edges matching its authored
    /// aliases, remote dependencies must have exact locks, and path edges must select the authored
    /// canonical directory.
    ///
    /// # Errors
    ///
    /// Returns an error for filesystem failures, invalid package data, duplicate identities or
    /// roots, unknown dependency identities, or an inconsistent resolved edge.
    pub fn load(specs: Vec<ResolvedPackageSpec>) -> Result<Self, PackageGraphError> {
        Self::load_with_source_overlay(specs, SourceOverlay::empty())
    }

    /// Loads exact packages through one accepted open-document overlay.
    ///
    /// The returned graph retains the overlay so source discovery cannot accidentally fall back to
    /// different bytes for the same compiler generation.
    ///
    /// # Errors
    ///
    /// Returns the same exact errors as [`Self::load`].
    pub fn load_with_source_overlay(
        specs: Vec<ResolvedPackageSpec>,
        source_overlay: SourceOverlay,
    ) -> Result<Self, PackageGraphError> {
        Self::load_with_root_catalog(
            specs,
            PackageRootCatalog::new(source_overlay),
            &mut DirectSourceSyntax,
        )
    }

    /// Loads exact packages while retaining package-root facts already selected for this overlay.
    ///
    /// # Errors
    ///
    /// Returns the same exact errors as [`Self::load_with_source_overlay`].
    pub fn load_with_root_catalog(
        mut specs: Vec<ResolvedPackageSpec>,
        package_roots: PackageRootCatalog,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<Self, PackageGraphError> {
        specs.sort_unstable_by(|left, right| left.identity.cmp(&right.identity));
        let mut identities = BTreeSet::new();
        for spec in &specs {
            if !identities.insert(spec.identity.clone()) {
                return Err(PackageGraphError::DuplicatePackage(spec.identity.clone()));
            }
        }
        let mut builder = PackageGraphBuilder::new(package_roots);
        let mut edges = BTreeMap::new();
        for spec in specs {
            let identity = spec.identity.clone();
            builder.load(spec.identity, &spec.root, source_syntax)?;
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
    pub const fn source_overlay(&self) -> &SourceOverlay {
        self.package_roots.source_overlay()
    }

    /// Renders one package declaration with its validated effective locks.
    ///
    /// Existing generated lock syntax is replaced as a unit. When no lock directive exists, one
    /// canonical sorted block is appended. No filesystem state is changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the package is absent or its retained source/syntax identity is
    /// internally inconsistent.
    pub fn root_lock_update(
        &self,
        identity: &PackageIdentity,
    ) -> Result<PackageLockSourceUpdate, PackageLockSourceError> {
        let package = self
            .packages
            .iter()
            .find(|package| package.identity() == identity)
            .ok_or(PackageLockSourceError::UnknownPackage)?;
        let syntax = self
            .syntax
            .get(package.declaration_syntax())
            .ok_or(PackageLockSourceError::MissingPackageSyntax)?;
        self.sources
            .get(syntax.root_id().source())
            .ok_or(PackageLockSourceError::MissingPackageSource)?;
        let declaration = package
            .declaration()
            .ok_or(PackageLockSourceError::MissingPackageDeclaration)?;
        crate::lock_source::render_effective_locks(
            package.declaration_path(),
            &package.root_source_bytes,
            syntax,
            declaration,
            package.locks(),
        )
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        PackageRootCatalog,
        SourceMap,
        Vec<SyntaxTree>,
        Vec<ResolvedPackageSnapshot>,
    ) {
        (self.package_roots, self.sources, self.syntax, self.packages)
    }
}

pub(crate) struct PackageGraphBuilder {
    package_roots: PackageRootCatalogBuilder,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    packages: BTreeMap<PackageIdentity, LoadedPackageSnapshot>,
    roots: BTreeMap<PathBuf, PackageIdentity>,
}

impl PackageGraphBuilder {
    pub(crate) fn new(package_roots: PackageRootCatalog) -> Self {
        Self {
            package_roots: package_roots.into_builder(),
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
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<(), PackageGraphError> {
        let canonical_root =
            canonical_package_root_with_overlay(self.package_roots.source_overlay(), root)?;
        self.load_canonical(identity, canonical_root, source_syntax)
    }

    pub(crate) fn load_canonical(
        &mut self,
        identity: PackageIdentity,
        canonical_root: PathBuf,
        source_syntax: &mut dyn SourceSyntaxProvider,
    ) -> Result<(), PackageGraphError> {
        if self.packages.contains_key(&identity) {
            return Err(PackageGraphError::DuplicatePackage(identity));
        }
        let package = load_package(
            &mut self.package_roots,
            identity.clone(),
            canonical_root,
            &mut self.roots,
            &mut self.sources,
            &mut self.syntax,
            source_syntax,
        )?;
        self.packages.insert(identity, package);
        Ok(())
    }

    pub(crate) fn declaration(&self, identity: &PackageIdentity) -> Option<&PackageDeclaration> {
        self.packages.get(identity)?.declaration.as_ref()
    }

    pub(crate) fn source_snapshot(&self) -> PackageSourceSnapshot {
        PackageSourceSnapshot {
            package_roots: self.package_roots.snapshot(),
            sources: self.sources.clone(),
            syntax: self.syntax.clone(),
        }
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
        validate_path_roots(self.package_roots.source_overlay(), &packages)?;
        Ok(ResolvedPackageGraph {
            package_roots: self.package_roots.finish(),
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
    root_source_bytes: Box<[u8]>,
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
            root_source_bytes: self.root_source_bytes,
            declaration_syntax: self.declaration_syntax,
            declaration: self.declaration,
        }
    }
}

fn load_package(
    package_roots: &mut PackageRootCatalogBuilder,
    identity: PackageIdentity,
    canonical_root: PathBuf,
    roots: &mut BTreeMap<PathBuf, PackageIdentity>,
    sources: &mut SourceMap,
    syntax: &mut Vec<SyntaxTree>,
    source_syntax: &mut dyn SourceSyntaxProvider,
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
    let Some(root_source) = package_roots
        .root_source_with_source_syntax(&canonical_root, source_syntax)
        .map_err(PackageGraphError::PackageRootProbe)?
    else {
        return Err(PackageGraphError::MissingPackageRootSource {
            package: identity,
            path: canonical_root.join("index.nct"),
        });
    };
    let declaration_path = root_source.path().to_path_buf();
    if !declaration_path.starts_with(&canonical_root) {
        return Err(PackageGraphError::InvalidPackageRoot {
            package: identity,
            path: declaration_path,
        });
    }
    let canonical_name = canonical_text(&declaration_path)?;
    let source_id = sources
        .add_bytes(
            SourceName::new(canonical_name.as_ref()),
            root_source.bytes(),
        )
        .map_err(|error| PackageGraphError::Source {
            path: declaration_path.clone(),
            error,
        })?;
    let source_file = sources
        .get(source_id)
        .expect("new package source remains in the source map");
    let tree = root_source
        .syntax()
        .bind(source_file)
        .ok_or_else(|| PackageGraphError::InconsistentRootSyntax(declaration_path.clone()))?;
    let declaration_syntax = syntax.len();
    syntax.push(tree);
    let tree = syntax
        .get(declaration_syntax)
        .expect("new package syntax remains in the syntax snapshot");
    let declaration = if tree.has_errors() {
        None
    } else {
        let source = sources
            .get(source_id)
            .expect("parsed package source remains in the source map");
        Some(decode_package_declaration(source, tree).map_err(PackageGraphError::Declaration)?)
    };
    let display_name = declaration
        .as_ref()
        .map(PackageDeclaration::name)
        .map_or_else(
            || directory_name(&canonical_root),
            |name| Ok(Box::<str>::from(name.value())),
        )?;
    Ok(LoadedPackageSnapshot {
        display_name,
        canonical_root,
        declaration_path,
        root_source_bytes: root_source.bytes().into(),
        declaration_syntax,
        declaration,
    })
}

#[cfg(test)]
pub(crate) fn canonical_package_root(path: &Path) -> Result<PathBuf, PackageGraphError> {
    canonical_package_root_with_overlay(&SourceOverlay::empty(), path)
}

pub(crate) fn canonical_package_root_with_overlay(
    source_overlay: &SourceOverlay,
    path: &Path,
) -> Result<PathBuf, PackageGraphError> {
    canonicalize(source_overlay, "canonicalize package root", path)
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

fn validate_path_roots(
    source_overlay: &SourceOverlay,
    packages: &[ResolvedPackageSnapshot],
) -> Result<(), PackageGraphError> {
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
                source_overlay,
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

fn canonicalize(
    source_overlay: &SourceOverlay,
    operation: &'static str,
    path: &Path,
) -> Result<PathBuf, PackageGraphError> {
    source_overlay
        .canonicalize(path)
        .map_err(|error| PackageGraphError::Filesystem {
            operation,
            path: path.into(),
            error,
        })
}

fn directory_name(path: &Path) -> Result<Box<str>, PackageGraphError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(Into::into)
        .ok_or_else(|| PackageGraphError::NonUnicodeCanonicalPath(path.into()))
}

fn canonical_text(path: &Path) -> Result<Box<str>, PackageGraphError> {
    path.to_str()
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
    MissingPackageRootSource {
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
    PackageRootProbe(Arc<PackageRootProbeError>),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    Source {
        path: PathBuf,
        error: SourceError,
    },
    InconsistentRootSyntax(PathBuf),
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
            Self::MissingPackageRootSource { package, path } => write!(
                formatter,
                "package {} has no package root source at {}",
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
            Self::PackageRootProbe(error) => error.fmt(formatter),
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
            Self::Source { path, error } => {
                write!(formatter, "cannot ingest {}: {error:?}", path.display())
            }
            Self::InconsistentRootSyntax(path) => write!(
                formatter,
                "package-root syntax does not match retained source {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PackageGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declaration(error) => Some(error),
            Self::PackageRootProbe(error) => Some(error.as_ref()),
            Self::Filesystem { error, .. } => Some(error),
            Self::DuplicatePackage(_)
            | Self::UnknownPackage(_)
            | Self::InvalidPackageRoot { .. }
            | Self::MissingPackageRootSource { .. }
            | Self::DuplicateCanonicalRoot { .. }
            | Self::DependencyEdgeMismatch { .. }
            | Self::MissingLock { .. }
            | Self::UnexpectedResolvedLock { .. }
            | Self::LockSelectionMismatch { .. }
            | Self::LockKindMismatch { .. }
            | Self::InvalidPathDependency { .. }
            | Self::InvalidImplicitDependency { .. }
            | Self::NonUnicodeCanonicalPath(_)
            | Self::Source { .. }
            | Self::InconsistentRootSyntax(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
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

    #[derive(Default)]
    struct CountingSourceSyntax {
        direct: DirectSourceSyntax,
        calls: usize,
    }

    impl SourceSyntaxProvider for CountingSourceSyntax {
        fn parsed_syntax(
            &mut self,
            source: &nocter_source::SourceFile,
            goal: nocter_syntax::ParseGoal,
        ) -> Result<Arc<nocter_syntax::ParsedSyntax>, nocter_syntax::SourceSyntaxError> {
            self.calls += 1;
            self.direct.parsed_syntax(source, goal)
        }
    }

    #[test]
    fn package_loading_reuses_a_topology_root_source() {
        let tree = TempTree::new();
        tree.source(
            "app/index.nct",
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        );
        let root = fs::canonicalize(tree.0.join("app")).unwrap();
        let mut catalog = PackageRootCatalogBuilder::new(SourceOverlay::empty());
        let mut source_syntax = CountingSourceSyntax::default();
        assert!(
            catalog
                .has_package_declaration(&root, &mut source_syntax)
                .unwrap()
        );

        let graph = ResolvedPackageGraph::load_with_root_catalog(
            vec![ResolvedPackageSpec::new(identity("app"), root)],
            catalog.finish(),
            &mut source_syntax,
        )
        .unwrap();

        assert_eq!(source_syntax.calls, 1);
        assert_eq!(graph.sources().len(), 1);
        assert_eq!(graph.syntax_trees().len(), 1);
    }

    #[test]
    fn loads_package_root_sources_and_closes_exact_path_edges() {
        let tree = TempTree::new();
        tree.source(
            "app/index.nct",
            "#package: { name: \"application\", version: \"0.0.0\", }\n#dependencies: { util: { path: \"../util\", }, }\n#executable: { name: \"app\", }\n",
        );
        tree.source(
            "util/index.nct",
            "#package: { name: \"utility\", version: \"0.0.0\", }\n",
        );
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
            "app/index.nct",
            "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n",
        );
        tree.source(
            "remote/index.nct",
            "#package: { name: \"remote\", version: \"0.0.0\", }\n",
        );
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

        tree.source(
            "empty/index.nct",
            "#package: { name: \"empty\", version: \"0.0.0\", }\n",
        );
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
            "app/index.nct",
            "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n",
        );
        tree.source(
            "remote/index.nct",
            "#package: { name: \"remote\", version: \"0.0.0\", }\n",
        );
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
            "app/index.nct",
            "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { remote: { git: \"https://example.test/r.git\", revision: \"main\", }, }\n#lock: { format: 1, dependencies: { remote: \"git:7db21c1000000000000000000000000000000000\", }, }\n",
        );
        tree.source(
            "remote/index.nct",
            "#package: { name: \"remote\", version: \"0.0.0\", }\n",
        );
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
    fn retains_syntax_invalid_root_sources_for_normal_diagnostic_projection() {
        let tree = TempTree::new();
        tree.source("app/index.nct", "#package: { name: { nested: \"app\"\n");
        let graph = ResolvedPackageGraph::load(vec![ResolvedPackageSpec::new(
            identity("app"),
            tree.0.join("app"),
        )])
        .unwrap();

        assert!(graph.syntax_trees()[0].has_errors());
        assert!(graph.packages()[0].declaration().is_none());
    }
}
