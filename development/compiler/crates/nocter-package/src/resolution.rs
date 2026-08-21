use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_model::PackageIdentity;

use crate::graph::{PackageGraphBuilder, ResolvedPackageEdges, canonical_package_root};
use crate::{DependencySource, PackageGraphError, PackageId, PackageIdError, ResolvedPackageGraph};

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
        }
    }
}

/// One complete graph with its non-inferable command-root and toolchain identities.
#[derive(Debug)]
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
/// Each selected `nocter.nct` is loaded exactly once into the returned graph. Missing mutable
/// state is reported as a typed lock or fetch requirement when policy permits it; a separate
/// package-management authority may satisfy that requirement and submit a new request.
///
/// # Errors
///
/// Returns an error for invalid package data, inconsistent identities, filesystem failures, or a
/// lock/fetch requirement that this read-only resolver cannot satisfy.
pub fn resolve_package_selection(
    request: PackageResolutionRequest,
) -> Result<ResolvedPackageSelection, PackageResolutionError> {
    let root = canonical_package_root(&request.root).map_err(PackageResolutionError::Graph)?;
    let root_id = PackageId::from_canonical_path(&root)
        .map_err(PackageResolutionError::PackageId)?
        .package_identity();
    let standard_root =
        canonical_package_root(&request.standard.root).map_err(PackageResolutionError::Graph)?;
    let standard_id = request.standard.identity;

    let mut builder = PackageGraphBuilder::new();
    let mut roots = BTreeMap::new();
    let mut pending = BTreeMap::new();
    insert_package(
        &mut builder,
        &mut roots,
        &mut pending,
        standard_id.clone(),
        standard_root,
    )?;
    insert_package(
        &mut builder,
        &mut roots,
        &mut pending,
        root_id.clone(),
        root.clone(),
    )?;

    let local_store = root.join(".nocter").join("packages");
    let home_store = request.nocter_home.join("packages");
    let dependency_resolver = DependencyResolver {
        local_store: &local_store,
        home_store: &home_store,
        policy: request.policy,
    };
    let mut edges = BTreeMap::new();
    while let Some((identity, package_root)) = pending.pop_first() {
        let declaration = builder.declaration(&identity).cloned();
        let mut authored = BTreeMap::new();
        if let Some(declaration) = declaration {
            for (alias, dependency) in declaration.dependencies() {
                let (target, target_root) = dependency_resolver.resolve(
                    &identity,
                    &package_root,
                    alias,
                    dependency.source(),
                    declaration.locks().get(alias),
                )?;
                authored.insert(alias.clone(), target.clone());
                if let Some(existing) = roots.get(&target) {
                    if existing != &target_root {
                        return Err(PackageResolutionError::IdentityRootConflict {
                            package: target,
                            first: existing.clone(),
                            second: target_root,
                        });
                    }
                } else {
                    insert_package(&mut builder, &mut roots, &mut pending, target, target_root)?;
                }
            }
        }
        let mut implicit = BTreeMap::new();
        implicit.insert("std".into(), standard_id.clone());
        edges.insert(identity, ResolvedPackageEdges { authored, implicit });
    }
    let graph = builder
        .finish(edges)
        .map_err(PackageResolutionError::Graph)?;
    Ok(ResolvedPackageSelection {
        graph,
        root: root_id,
        standard: standard_id,
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
pub fn resolve_package_graph(
    request: PackageResolutionRequest,
) -> Result<ResolvedPackageGraph, PackageResolutionError> {
    let (graph, _, _) = resolve_package_selection(request)?.into_parts();
    Ok(graph)
}

/// Loads the self-contained standard package selected by a toolchain for single-file mode.
///
/// # Errors
///
/// Returns a graph error if the package is invalid or declares an authored dependency. Bundled
/// standard libraries are closed toolchain inputs and never resolve through user package stores.
pub fn resolve_standard_package(
    standard: StandardPackage,
) -> Result<ResolvedPackageGraph, PackageGraphError> {
    let identity = standard.identity;
    ResolvedPackageGraph::load(vec![
        crate::ResolvedPackageSpec::new(identity.clone(), standard.root)
            .with_standard_dependency(identity),
    ])
}

fn insert_package(
    builder: &mut PackageGraphBuilder,
    roots: &mut BTreeMap<PackageIdentity, PathBuf>,
    pending: &mut BTreeMap<PackageIdentity, PathBuf>,
    identity: PackageIdentity,
    root: PathBuf,
) -> Result<(), PackageResolutionError> {
    if let Some(first) = roots.get(&identity) {
        return Err(PackageResolutionError::IdentityRootConflict {
            package: identity,
            first: first.clone(),
            second: root,
        });
    }
    builder
        .load_canonical(identity.clone(), root.clone())
        .map_err(PackageResolutionError::Graph)?;
    roots.insert(identity.clone(), root.clone());
    pending.insert(identity, root);
    Ok(())
}

struct DependencyResolver<'a> {
    local_store: &'a Path,
    home_store: &'a Path,
    policy: PackageResolutionPolicy,
}

impl DependencyResolver<'_> {
    fn resolve(
        &self,
        package: &PackageIdentity,
        package_root: &Path,
        alias: &str,
        source: &DependencySource,
        lock: Option<&crate::DependencyLock>,
    ) -> Result<(PackageIdentity, PathBuf), PackageResolutionError> {
        if let DependencySource::Path { path } = source {
            let root = canonical_package_root(&package_root.join(path.value()))
                .map_err(PackageResolutionError::Graph)?;
            let identity = PackageId::from_canonical_path(&root)
                .map_err(PackageResolutionError::PackageId)?
                .package_identity();
            return Ok((identity, root));
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
                alias: alias.into(),
                source: source.clone(),
            });
        };
        let package_id = PackageId::from_lock(lock).map_err(PackageResolutionError::PackageId)?;
        let selected = self.select_installed(&package_id)?;
        if let Some(root) = selected {
            return Ok((
                package_id.into(),
                canonical_package_root(&root).map_err(PackageResolutionError::Graph)?,
            ));
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
            source: source.clone(),
        })
    }

    fn select_installed(
        &self,
        package: &PackageId,
    ) -> Result<Option<PathBuf>, PackageResolutionError> {
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
pub enum PackageResolutionError {
    LockRequired {
        package: PackageIdentity,
        alias: Box<str>,
        source: DependencySource,
    },
    FetchRequired {
        package: PackageIdentity,
        alias: Box<str>,
        package_id: PackageId,
        source: DependencySource,
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
            | Self::IdentityRootConflict { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

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

    fn root_manifest(lock: bool) -> String {
        let lock = if lock {
            format!("#lock: {{ format: 1, dependencies: {{ remote: \"git:{COMMIT}\", }}, }}\n")
        } else {
            String::new()
        };
        format!(
            "#name: \"app\"\n#dependencies: {{ remote: {{ git: \"https://example.test/repository.git\", revision: \"main\", }}, local: {{ path: \"../local\", }}, }}\n{lock}"
        )
    }

    fn base_tree(lock: bool) -> TempTree {
        let tree = TempTree::new();
        tree.source("app/nocter.nct", &root_manifest(lock));
        tree.source("local/nocter.nct", "#name: \"local\"\n");
        tree.source("std/nocter.nct", "#name: \"std\"\n");
        tree
    }

    #[test]
    fn resolves_local_store_before_home_and_closes_path_and_standard_edges() {
        let tree = base_tree(true);
        let package_id = PackageId::from_git_commit(COMMIT).unwrap();
        tree.source(
            &format!("app/.nocter/packages/{}/nocter.nct", package_id.as_str()),
            "#name: \"package-local-remote\"\n",
        );
        tree.source(
            &format!("home/packages/{}/nocter.nct", package_id.as_str()),
            "#name: \"home-remote\"\n",
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
}
