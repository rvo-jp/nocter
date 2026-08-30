use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_filesystem::SourceOverlay;
use nocter_model::PackageIdentity;
#[cfg(test)]
use nocter_syntax::DirectSourceSyntax;
use nocter_syntax::SourceSyntaxProvider;

#[cfg(test)]
use crate::graph::canonical_package_root;
use crate::graph::{
    PackageGraphBuilder, ResolvedPackageEdges, canonical_package_root_with_overlay,
};
use crate::{
    DependencySource, ExactDependencyLock, PackageGraphError, PackageId, PackageIdError,
    PackageLockOverlay, PackageRootCatalog, PackageSourceSnapshot, PackageStoreOverlay,
    ResolvedPackageGraph,
};

/// Immutable policy controlling whether exact resolution may request lock or fetch authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PackageResolutionPolicy {
    locked: bool,
    offline: bool,
}

impl PackageResolutionPolicy {
    #[must_use]
    pub const fn new(locked: bool, offline: bool) -> Self {
        Self { locked, offline }
    }

    #[must_use]
    pub const fn locked(self) -> bool {
        self.locked
    }

    #[must_use]
    pub const fn offline(self) -> bool {
        self.offline
    }
}

/// Exact standard-library package selected by the active toolchain.
#[derive(Clone, Debug)]
pub struct StandardPackage {
    identity: PackageIdentity,
    root: PathBuf,
}

impl StandardPackage {
    #[must_use]
    pub fn new(identity: PackageIdentity, root: impl Into<PathBuf>) -> Self {
        Self {
            identity,
            root: root.into(),
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
}

/// Complete process-independent input to exact package graph resolution.
#[derive(Clone, Debug)]
pub struct PackageResolutionRequest {
    root: PathBuf,
    nocter_home: PathBuf,
    standard: StandardPackage,
    policy: PackageResolutionPolicy,
    lock_overlay: PackageLockOverlay,
    store_overlay: PackageStoreOverlay,
}

impl PackageResolutionRequest {
    #[must_use]
    pub fn new(
        root: impl Into<PathBuf>,
        nocter_home: impl Into<PathBuf>,
        standard: StandardPackage,
        policy: PackageResolutionPolicy,
    ) -> Self {
        Self {
            root: root.into(),
            nocter_home: nocter_home.into(),
            standard,
            policy,
            lock_overlay: PackageLockOverlay::new(),
            store_overlay: PackageStoreOverlay::new(),
        }
    }

    /// Supplies exact selections created by a package-state transaction but not yet committed to
    /// package source.
    #[must_use]
    pub fn with_lock_overlay(mut self, lock_overlay: PackageLockOverlay) -> Self {
        self.lock_overlay = lock_overlay;
        self
    }

    /// Supplies exact package roots staged by a package-state transaction but not yet published
    /// to a persistent store.
    #[must_use]
    pub fn with_store_overlay(mut self, store_overlay: PackageStoreOverlay) -> Self {
        self.store_overlay = store_overlay;
        self
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn nocter_home(&self) -> &Path {
        &self.nocter_home
    }

    #[must_use]
    pub const fn standard(&self) -> &StandardPackage {
        &self.standard
    }

    #[must_use]
    pub const fn policy(&self) -> PackageResolutionPolicy {
        self.policy
    }
}

/// One complete graph with its non-inferable command-root and toolchain identities.
#[derive(Clone, Debug)]
pub struct ResolvedPackageSelection {
    graph: ResolvedPackageGraph,
    root: PackageIdentity,
    standard: PackageIdentity,
}

impl ResolvedPackageSelection {
    #[must_use]
    pub const fn graph(&self) -> &ResolvedPackageGraph {
        &self.graph
    }

    #[must_use]
    pub const fn root(&self) -> &PackageIdentity {
        &self.root
    }

    #[must_use]
    pub const fn standard(&self) -> &PackageIdentity {
        &self.standard
    }

    #[must_use]
    pub fn into_parts(self) -> (ResolvedPackageGraph, PackageIdentity, PackageIdentity) {
        (self.graph, self.root, self.standard)
    }
}

/// Resolves one root package and its complete exact dependency selection without mutating locks
/// or stores.
///
/// Each selected `index.nct` is loaded exactly once into the returned graph. Missing mutable
/// state is reported as a typed lock or fetch requirement when policy permits it; a separate
/// package-management authority may satisfy that requirement and submit a new request.
///
/// # Errors
///
/// Returns an error for invalid package data, inconsistent identities, filesystem failures, or a
/// lock/fetch requirement that this read-only resolver cannot satisfy.
#[cfg(test)]
pub fn resolve_package_selection(
    request: PackageResolutionRequest,
) -> Result<ResolvedPackageSelection, PackageResolutionError> {
    resolve_package_selection_with_source_overlay(request, SourceOverlay::empty())
}

/// Resolves through an immutable open-document view without granting package mutation authority.
///
/// The returned graph retains `source_overlay` for later module discovery. Package-state
/// transactions deliberately accept only [`PackageResolutionRequest`], so editor bytes cannot be
/// mistaken for disk bytes during lock generation or publication.
///
/// # Errors
///
/// Returns the same exact resolution errors as [`resolve_package_selection`].
#[cfg(test)]
pub fn resolve_package_selection_with_source_overlay(
    request: PackageResolutionRequest,
    source_overlay: SourceOverlay,
) -> Result<ResolvedPackageSelection, PackageResolutionError> {
    resolve_package_selection_with_source_snapshot(request, source_overlay)
        .map_err(PackageResolutionFailure::into_error)
}

/// Resolves through an immutable open-document view and retains every package source and syntax
/// tree reached before failure.
///
/// # Errors
///
/// Returns the same exact resolution error together with its immutable reached-source snapshot.
#[cfg(test)]
pub fn resolve_package_selection_with_source_snapshot(
    request: PackageResolutionRequest,
    source_overlay: SourceOverlay,
) -> Result<ResolvedPackageSelection, PackageResolutionFailure> {
    resolve_package_selection_with_root_catalog(
        request,
        PackageRootCatalog::new(source_overlay),
        &mut DirectSourceSyntax,
    )
}

/// Resolves through package-root facts already selected from one immutable source view.
///
/// # Errors
///
/// Returns the exact package-domain rejection together with the immutable source snapshot reached
/// through the supplied catalog and syntax provider.
pub fn resolve_package_selection_with_root_catalog(
    request: PackageResolutionRequest,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<ResolvedPackageSelection, PackageResolutionFailure> {
    let empty_snapshot = || PackageSourceSnapshot::from_root_catalog(package_roots.clone());
    let PackageResolutionRequest {
        root: requested_root,
        nocter_home,
        standard,
        policy,
        lock_overlay,
        store_overlay,
    } = request;
    let source_overlay = package_roots.source_overlay();
    let root = canonical_package_root_with_overlay(source_overlay, &requested_root)
        .map_err(PackageResolutionError::Graph)
        .map_err(|error| PackageResolutionFailure::new(error, empty_snapshot()))?;
    let root_id = PackageId::from_canonical_path(&root)
        .map_err(PackageResolutionError::PackageId)
        .map_err(|error| PackageResolutionFailure::new(error, empty_snapshot()))?
        .package_identity();
    let standard_root = canonical_package_root_with_overlay(source_overlay, &standard.root)
        .map_err(PackageResolutionError::Graph)
        .map_err(|error| PackageResolutionFailure::new(error, empty_snapshot()))?;
    let standard_id = standard.identity;

    let source_overlay_for_resolution = source_overlay.clone();
    let mut builder = PackageGraphBuilder::new(package_roots);
    let mut roots = BTreeMap::new();
    let mut pending = BTreeMap::new();
    if let Err(error) = insert_package(
        &mut builder,
        &mut roots,
        &mut pending,
        standard_id.clone(),
        standard_root,
        source_syntax,
    ) {
        return Err(PackageResolutionFailure::from_builder(error, &builder));
    }
    if let Err(error) = insert_package(
        &mut builder,
        &mut roots,
        &mut pending,
        root_id.clone(),
        root.clone(),
        source_syntax,
    ) {
        return Err(PackageResolutionFailure::from_builder(error, &builder));
    }

    let local_store = root.join(".nocter").join("packages");
    let home_store = nocter_home.join("packages");
    let dependency_resolver = DependencyResolver {
        local_store: &local_store,
        home_store: &home_store,
        policy,
        store_overlay: &store_overlay,
        source_overlay: &source_overlay_for_resolution,
    };
    let mut edges = BTreeMap::new();
    while let Some((identity, package_root)) = pending.pop_first() {
        let declaration = builder.declaration(&identity).cloned();
        let resolved = resolve_package_edges(
            &identity,
            &package_root,
            declaration.as_ref(),
            &dependency_resolver,
            &lock_overlay,
            &standard_id,
        );
        let resolved =
            resolved.map_err(|error| PackageResolutionFailure::from_builder(error, &builder))?;
        for dependency in resolved.pending {
            if let Some(existing) = roots.get(&dependency.target) {
                if existing != &dependency.root {
                    return Err(PackageResolutionFailure::from_builder(
                        PackageResolutionError::IdentityRootConflict {
                            package: dependency.target,
                            first: existing.clone(),
                            second: dependency.root,
                        },
                        &builder,
                    ));
                }
            } else if let Err(error) = insert_package(
                &mut builder,
                &mut roots,
                &mut pending,
                dependency.target,
                dependency.root,
                source_syntax,
            ) {
                return Err(PackageResolutionFailure::from_builder(error, &builder));
            }
        }
        edges.insert(identity, resolved.edges);
    }
    finish_resolution(builder, edges, root_id, standard_id)
}

fn finish_resolution(
    builder: PackageGraphBuilder,
    edges: BTreeMap<PackageIdentity, ResolvedPackageEdges>,
    root: PackageIdentity,
    standard: PackageIdentity,
) -> Result<ResolvedPackageSelection, PackageResolutionFailure> {
    let reached = builder.source_snapshot();
    let graph = builder
        .finish(edges)
        .map_err(PackageResolutionError::Graph)
        .map_err(|error| PackageResolutionFailure::new(error, reached))?;
    Ok(ResolvedPackageSelection {
        graph,
        root,
        standard,
    })
}

struct ResolvedPackageWork {
    edges: ResolvedPackageEdges,
    pending: Vec<ResolvedDependency>,
}

fn resolve_package_edges(
    identity: &PackageIdentity,
    package_root: &Path,
    declaration: Option<&crate::PackageDeclaration>,
    resolver: &DependencyResolver<'_>,
    overlay: &PackageLockOverlay,
    standard: &PackageIdentity,
) -> Result<ResolvedPackageWork, PackageResolutionError> {
    let mut authored = BTreeMap::new();
    let mut locks = BTreeMap::new();
    let mut pending = Vec::new();
    if let Some(declaration) = declaration {
        for (alias, dependency) in declaration.dependencies() {
            let authored_lock = dependency
                .selection()
                .map(crate::DependencyExactSelection::exact);
            let overlay_lock = overlay.get(identity, alias);
            if authored_lock
                .as_ref()
                .zip(overlay_lock)
                .is_some_and(|(authored, overlay)| authored != overlay)
            {
                return Err(PackageResolutionError::LockOverrideConflict {
                    package: identity.clone(),
                    alias: alias.clone(),
                });
            }
            let effective_lock = overlay_lock.or(authored_lock.as_ref());
            let mut selection = resolver.resolve(
                identity,
                package_root,
                alias,
                dependency.source(),
                effective_lock,
            )?;
            authored.insert(alias.clone(), selection.target.clone());
            if let Some(lock) = selection.lock.take() {
                locks.insert(alias.clone(), lock);
            }
            pending.push(selection);
        }
    }
    let mut implicit = BTreeMap::new();
    implicit.insert("std".into(), standard.clone());
    Ok(ResolvedPackageWork {
        edges: ResolvedPackageEdges {
            authored,
            locks,
            implicit,
        },
        pending,
    })
}

/// Resolves one root package and returns only its exact graph.
///
/// Prefer [`resolve_package_selection`] when a later stage needs to distinguish the command root
/// from its dependencies.
///
/// # Errors
///
/// Returns the same exact resolution error as [`resolve_package_selection`].
#[cfg(test)]
pub fn resolve_package_graph(
    request: PackageResolutionRequest,
) -> Result<ResolvedPackageGraph, PackageResolutionError> {
    let (graph, _, _) = resolve_package_selection(request)?.into_parts();
    Ok(graph)
}

/// Resolves one exact package graph through an immutable open-document view.
///
/// # Errors
///
/// Returns the same exact resolution errors as [`resolve_package_graph`].
#[cfg(test)]
pub fn resolve_package_graph_with_source_overlay(
    request: PackageResolutionRequest,
    source_overlay: SourceOverlay,
) -> Result<ResolvedPackageGraph, PackageResolutionError> {
    let (graph, _, _) =
        resolve_package_selection_with_source_overlay(request, source_overlay)?.into_parts();
    Ok(graph)
}

/// Loads the selected standard package from an existing package-root catalog.
///
/// # Errors
///
/// Returns graph errors when the package is invalid or declares an authored dependency.
pub fn resolve_standard_package_with_root_catalog(
    standard: StandardPackage,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<ResolvedPackageGraph, PackageGraphError> {
    let identity = standard.identity;
    ResolvedPackageGraph::load_with_root_catalog(
        vec![
            crate::ResolvedPackageSpec::new(identity.clone(), standard.root)
                .with_standard_dependency(identity),
        ],
        package_roots,
        source_syntax,
    )
}

fn insert_package(
    builder: &mut PackageGraphBuilder,
    roots: &mut BTreeMap<PackageIdentity, PathBuf>,
    pending: &mut BTreeMap<PackageIdentity, PathBuf>,
    identity: PackageIdentity,
    root: PathBuf,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<(), PackageResolutionError> {
    if let Some(first) = roots.get(&identity) {
        return Err(PackageResolutionError::IdentityRootConflict {
            package: identity,
            first: first.clone(),
            second: root,
        });
    }
    builder
        .load_canonical(identity.clone(), root.clone(), source_syntax)
        .map_err(PackageResolutionError::Graph)?;
    roots.insert(identity.clone(), root.clone());
    pending.insert(identity, root);
    Ok(())
}

struct DependencyResolver<'a> {
    local_store: &'a Path,
    home_store: &'a Path,
    policy: PackageResolutionPolicy,
    store_overlay: &'a PackageStoreOverlay,
    source_overlay: &'a SourceOverlay,
}

struct ResolvedDependency {
    target: PackageIdentity,
    root: PathBuf,
    lock: Option<ExactDependencyLock>,
}

impl DependencyResolver<'_> {
    fn resolve(
        &self,
        package: &PackageIdentity,
        package_root: &Path,
        alias: &str,
        source: &DependencySource,
        lock: Option<&ExactDependencyLock>,
    ) -> Result<ResolvedDependency, PackageResolutionError> {
        if let DependencySource::Path { path } = source {
            let root = canonical_package_root_with_overlay(
                self.source_overlay,
                &package_root.join(path.value()),
            )
            .map_err(PackageResolutionError::Graph)?;
            let target = PackageId::from_canonical_path(&root)
                .map_err(PackageResolutionError::PackageId)?
                .package_identity();
            return Ok(ResolvedDependency {
                target,
                root,
                lock: None,
            });
        }

        let Some(lock) = lock else {
            if self.policy.locked() {
                return Err(PackageResolutionError::MissingLockLocked {
                    package: package.clone(),
                    alias: alias.into(),
                });
            }
            if self.policy.offline() {
                return Err(PackageResolutionError::MissingLockOffline {
                    package: package.clone(),
                    alias: alias.into(),
                });
            }
            return Err(PackageResolutionError::LockRequired {
                package: package.clone(),
                package_root: package_root.into(),
                alias: alias.into(),
                source: Box::new(source.clone()),
            });
        };
        let package_id =
            PackageId::from_exact_lock(lock).map_err(PackageResolutionError::PackageId)?;
        let selected = self.select_installed(&package_id)?;
        if let Some(root) = selected {
            return Ok(ResolvedDependency {
                target: package_id.into(),
                root: canonical_package_root_with_overlay(self.source_overlay, &root)
                    .map_err(PackageResolutionError::Graph)?,
                lock: Some(lock.clone()),
            });
        }
        if self.policy.offline() {
            return Err(PackageResolutionError::PackageUnavailableOffline {
                package: package.clone(),
                alias: alias.into(),
                package_id,
            });
        }
        Err(PackageResolutionError::FetchRequired {
            package: package.clone(),
            alias: alias.into(),
            package_id,
            lock: lock.clone(),
            source: Box::new(source.clone()),
        })
    }

    fn select_installed(
        &self,
        package: &PackageId,
    ) -> Result<Option<PathBuf>, PackageResolutionError> {
        if let Some(root) = self.store_overlay.get(package) {
            return Ok(Some(root.into()));
        }
        for store in [self.local_store, self.home_store] {
            let candidate = store.join(package.as_str());
            match fs::metadata(&candidate) {
                Ok(_) => return Ok(Some(candidate)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(PackageResolutionError::Filesystem {
                        operation: "inspect exact package",
                        path: candidate,
                        error,
                    });
                }
            }
        }
        Ok(None)
    }
}

#[derive(Debug)]
pub struct PackageResolutionFailure {
    error: Box<PackageResolutionError>,
    reached: PackageSourceSnapshot,
}

impl PackageResolutionFailure {
    fn new(error: PackageResolutionError, reached: PackageSourceSnapshot) -> Self {
        Self {
            error: Box::new(error),
            reached,
        }
    }

    fn from_builder(error: PackageResolutionError, builder: &PackageGraphBuilder) -> Self {
        Self::new(error, builder.source_snapshot())
    }

    #[must_use]
    pub const fn error(&self) -> &PackageResolutionError {
        &self.error
    }

    #[must_use]
    pub const fn reached(&self) -> &PackageSourceSnapshot {
        &self.reached
    }

    #[must_use]
    pub fn into_error(self) -> PackageResolutionError {
        *self.error
    }
}

impl fmt::Display for PackageResolutionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for PackageResolutionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.error)
    }
}

#[derive(Debug)]
pub enum PackageResolutionError {
    LockRequired {
        package: PackageIdentity,
        package_root: PathBuf,
        alias: Box<str>,
        source: Box<DependencySource>,
    },
    FetchRequired {
        package: PackageIdentity,
        alias: Box<str>,
        package_id: PackageId,
        lock: ExactDependencyLock,
        source: Box<DependencySource>,
    },
    MissingLockLocked {
        package: PackageIdentity,
        alias: Box<str>,
    },
    MissingLockOffline {
        package: PackageIdentity,
        alias: Box<str>,
    },
    PackageUnavailableOffline {
        package: PackageIdentity,
        alias: Box<str>,
        package_id: PackageId,
    },
    IdentityRootConflict {
        package: PackageIdentity,
        first: PathBuf,
        second: PathBuf,
    },
    LockOverrideConflict {
        package: PackageIdentity,
        alias: Box<str>,
    },
    PackageId(PackageIdError),
    Graph(PackageGraphError),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
}

impl fmt::Display for PackageResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockRequired { package, alias, .. } => write!(
                formatter,
                "package {} dependency {alias} requires an exact lock",
                package.as_str()
            ),
            Self::FetchRequired {
                package,
                alias,
                package_id,
                ..
            } => write!(
                formatter,
                "package {} dependency {alias} requires fetching {}",
                package.as_str(),
                package_id.as_str()
            ),
            Self::MissingLockLocked { package, alias } => write!(
                formatter,
                "package {} dependency {alias} has no lock under locked resolution",
                package.as_str()
            ),
            Self::MissingLockOffline { package, alias } => write!(
                formatter,
                "package {} dependency {alias} cannot resolve a lock offline",
                package.as_str()
            ),
            Self::PackageUnavailableOffline {
                package,
                alias,
                package_id,
            } => write!(
                formatter,
                "package {} dependency {alias} exact package {} is unavailable offline",
                package.as_str(),
                package_id.as_str()
            ),
            Self::IdentityRootConflict {
                package,
                first,
                second,
            } => write!(
                formatter,
                "package {} selects both {} and {}",
                package.as_str(),
                first.display(),
                second.display()
            ),
            Self::LockOverrideConflict { package, alias } => write!(
                formatter,
                "package {} dependency {alias} has conflicting authored and transaction locks",
                package.as_str()
            ),
            Self::PackageId(error) => error.fmt(formatter),
            Self::Graph(error) => error.fmt(formatter),
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for PackageResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PackageId(error) => Some(error),
            Self::Graph(error) => Some(error),
            Self::Filesystem { error, .. } => Some(error),
            Self::LockRequired { .. }
            | Self::FetchRequired { .. }
            | Self::MissingLockLocked { .. }
            | Self::MissingLockOffline { .. }
            | Self::PackageUnavailableOffline { .. }
            | Self::IdentityRootConflict { .. }
            | Self::LockOverrideConflict { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    const COMMIT: &str = "7db21c1000000000000000000000000000000000";

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-package-resolution-{}-{serial}",
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

        fn request(&self, policy: PackageResolutionPolicy) -> PackageResolutionRequest {
            PackageResolutionRequest::new(
                self.0.join("app"),
                self.0.join("home"),
                StandardPackage::new(PackageIdentity::new("toolchain-std"), self.0.join("std")),
                policy,
            )
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn root_source(lock: bool) -> String {
        let exact = if lock {
            format!(" commit: \"{COMMIT}\",")
        } else {
            String::new()
        };
        format!(
            "#package: {{ name: \"app\", version: \"0.0.0\", }}\n#dependencies: {{ remote: {{ git: \"https://example.test/repository.git\", revision: \"main\",{exact} }}, local: {{ path: \"../local\", }}, }}\n"
        )
    }

    fn base_tree(lock: bool) -> TempTree {
        let tree = TempTree::new();
        tree.source("app/index.nct", &root_source(lock));
        tree.source(
            "local/index.nct",
            "#package: { name: \"local\", version: \"0.0.0\", }\n",
        );
        tree.source(
            "std/index.nct",
            "#package: { name: \"std\", version: \"0.0.0\", }\n",
        );
        tree
    }

    #[test]
    fn resolves_local_store_before_home_and_closes_path_and_standard_edges() {
        let tree = base_tree(true);
        let package_id = PackageId::from_git_commit(COMMIT).unwrap();
        tree.source(
            &format!("app/.nocter/packages/{}/index.nct", package_id.as_str()),
            "#package: { name: \"package-local-remote\", version: \"0.0.0\", }\n",
        );
        tree.source(
            &format!("home/packages/{}/index.nct", package_id.as_str()),
            "#package: { name: \"home-remote\", version: \"0.0.0\", }\n",
        );

        let graph =
            resolve_package_graph(tree.request(PackageResolutionPolicy::default())).unwrap();
        assert_eq!(graph.packages().len(), 4);
        assert!(
            graph
                .packages()
                .iter()
                .any(|package| package.display_name() == "package-local-remote")
        );
        assert!(
            !graph
                .packages()
                .iter()
                .any(|package| package.display_name() == "home-remote")
        );
        for package in graph.packages() {
            assert!(package.dependencies().contains_key("std"));
        }
    }

    #[test]
    fn package_resolution_retains_the_exact_open_root_source_overlay() {
        let tree = base_tree(true);
        let package_id = PackageId::from_git_commit(COMMIT).unwrap();
        tree.source(
            &format!("app/.nocter/packages/{}/index.nct", package_id.as_str()),
            "#package: { name: \"remote\", version: \"0.0.0\", }\n",
        );
        let root_source_path = fs::canonicalize(tree.0.join("app/index.nct")).unwrap();
        let mut overlay = SourceOverlay::builder();
        overlay
            .insert_document(
                root_source_path.clone(),
                OpenDocument::new(
                    DocumentVersion::new(12),
                    root_source(true)
                        .replace(
                            "#package: { name: \"app\", version: \"0.0.0\", }",
                            "#package: { name: \"editor-app\", version: \"0.0.0\", }",
                        )
                        .into_bytes(),
                ),
            )
            .unwrap();

        let graph = resolve_package_graph_with_source_overlay(
            tree.request(PackageResolutionPolicy::default()),
            overlay.finish(),
        )
        .unwrap();

        assert_eq!(
            graph
                .source_overlay()
                .document(&root_source_path)
                .unwrap()
                .version(),
            DocumentVersion::new(12)
        );
        assert!(
            graph
                .packages()
                .iter()
                .any(|package| package.display_name() == "editor-app")
        );
    }

    #[test]
    fn missing_mutable_state_is_a_typed_policy_boundary() {
        let unlocked = base_tree(false);
        assert!(matches!(
            resolve_package_graph(unlocked.request(PackageResolutionPolicy::default())),
            Err(PackageResolutionError::LockRequired { .. })
        ));
        assert!(matches!(
            resolve_package_graph(unlocked.request(PackageResolutionPolicy::new(true, false))),
            Err(PackageResolutionError::MissingLockLocked { .. })
        ));

        let locked = base_tree(true);
        assert!(matches!(
            resolve_package_graph(locked.request(PackageResolutionPolicy::default())),
            Err(PackageResolutionError::FetchRequired { .. })
        ));
        assert!(matches!(
            resolve_package_graph(locked.request(PackageResolutionPolicy::new(false, true))),
            Err(PackageResolutionError::PackageUnavailableOffline { .. })
        ));
    }

    #[test]
    fn resolution_failure_retains_every_root_source_reached_before_policy_rejection() {
        let tree = base_tree(false);

        let failure = resolve_package_selection_with_source_snapshot(
            tree.request(PackageResolutionPolicy::new(true, true)),
            SourceOverlay::empty(),
        )
        .unwrap_err();

        assert!(matches!(
            failure.error(),
            PackageResolutionError::MissingLockLocked { .. }
        ));
        assert_eq!(failure.reached().sources().len(), 2);
        assert_eq!(failure.reached().syntax_trees().len(), 2);
    }

    #[test]
    fn declaration_failure_retains_the_tree_that_owns_its_subject() {
        let tree = base_tree(false);
        tree.source(
            "app/index.nct",
            "#dependencies: { remote: { unknown: \"value\", }, }\n",
        );

        let failure = resolve_package_selection_with_source_snapshot(
            tree.request(PackageResolutionPolicy::new(true, true)),
            SourceOverlay::empty(),
        )
        .unwrap_err();

        assert!(matches!(
            failure.error(),
            PackageResolutionError::Graph(PackageGraphError::Declaration(_))
        ));
        assert_eq!(failure.reached().sources().len(), 2);
        assert_eq!(failure.reached().syntax_trees().len(), 2);
    }

    #[test]
    fn resolves_with_a_provisional_lock_without_editing_the_root_source() {
        let tree = base_tree(false);
        let package_id = PackageId::from_git_commit(COMMIT).unwrap();
        tree.source(
            &format!("app/.nocter/packages/{}/index.nct", package_id.as_str()),
            "#package: { name: \"remote\", version: \"0.0.0\", }\n",
        );
        let root_source_path = tree.0.join("app/index.nct");
        let before = fs::read(&root_source_path).unwrap();
        let root = canonical_package_root(&tree.0.join("app")).unwrap();
        let root_id = PackageId::from_canonical_path(&root)
            .unwrap()
            .package_identity();
        let lock = ExactDependencyLock::git(COMMIT).unwrap();
        let mut overlay = PackageLockOverlay::new();
        overlay
            .insert(root_id.clone(), "remote", lock.clone())
            .unwrap();

        let graph = resolve_package_graph(
            tree.request(PackageResolutionPolicy::default())
                .with_lock_overlay(overlay),
        )
        .unwrap();

        let root = graph
            .packages()
            .iter()
            .find(|package| package.identity() == &root_id)
            .unwrap();
        assert_eq!(root.locks().get("remote"), Some(&lock));
        assert!(
            root.declaration().unwrap().dependencies()["remote"]
                .selection()
                .is_none()
        );
        assert_eq!(fs::read(root_source_path).unwrap(), before);
    }

    #[test]
    fn resolves_a_staged_package_without_publishing_it_to_a_store() {
        let tree = base_tree(true);
        let package_id = PackageId::from_git_commit(COMMIT).unwrap();
        let staged = tree.0.join("staging/remote");
        tree.source(
            "staging/remote/index.nct",
            "#package: { name: \"staged-remote\", version: \"0.0.0\", }\n",
        );
        let mut overlay = PackageStoreOverlay::new();
        overlay.insert(package_id.clone(), &staged).unwrap();

        let graph = resolve_package_graph(
            tree.request(PackageResolutionPolicy::default())
                .with_store_overlay(overlay),
        )
        .unwrap();

        let remote = graph
            .packages()
            .iter()
            .find(|package| package.identity() == &package_id.package_identity())
            .unwrap();
        assert_eq!(remote.root(), canonical_package_root(&staged).unwrap());
        assert!(!tree.0.join("app/.nocter/packages").exists());
    }
}
