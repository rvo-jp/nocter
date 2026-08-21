use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_model::PackageIdentity;
use nocter_package::{
    ExactDependencyLock, PackageId, PackageResolutionError, PackageResolutionPolicy,
    PackageResolutionRequest, ResolvedPackageGraph, ResolvedPackageSpec, StandardPackage,
};

use crate::manifest::{ManifestCommitError, commit_lock_source};
use crate::{
    LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest, PackageStateError,
    resolve_package_state,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const COMMIT: &str = "7db21c1000000000000000000000000000000000";

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nocter-package-state-{}-{serial}",
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

struct FakeAuthority {
    manifest: Box<str>,
    lock_calls: usize,
    fetch_calls: usize,
}

impl FakeAuthority {
    fn new(manifest: &str) -> Self {
        Self {
            manifest: manifest.into(),
            lock_calls: 0,
            fetch_calls: 0,
        }
    }
}

impl PackageAcquisitionAuthority for FakeAuthority {
    type Error = io::Error;

    fn resolve_lock(
        &mut self,
        request: LockResolutionRequest<'_>,
    ) -> Result<ExactDependencyLock, Self::Error> {
        assert_eq!(request.alias(), "remote");
        self.lock_calls += 1;
        ExactDependencyLock::git(COMMIT)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn fetch_package(&mut self, request: PackageFetchRequest<'_>) -> Result<(), Self::Error> {
        assert_eq!(request.alias(), "remote");
        assert_eq!(request.lock().value(), COMMIT);
        self.fetch_calls += 1;
        fs::write(request.destination().join("nocter.nct"), &*self.manifest)
    }
}

fn base_tree() -> TempTree {
    let tree = TempTree::new();
    tree.source(
        "app/nocter.nct",
        "#name: \"app\"\n#dependencies: { remote: { git: \"https://example.test/remote.git\", revision: \"main\", }, }\n",
    );
    tree.source("std/nocter.nct", "#name: \"std\"\n");
    tree
}

#[test]
fn validates_staging_before_publishing_and_commits_the_root_lock_last() {
    let tree = base_tree();
    let mut authority = FakeAuthority::new("#name: \"remote\"\n");

    let selected = resolve_package_state(
        tree.request(PackageResolutionPolicy::default()),
        &mut authority,
    )
    .unwrap();

    let package = PackageId::from_git_commit(COMMIT).unwrap();
    let stored = tree.0.join("app/.nocter/packages").join(package.as_str());
    assert_eq!(authority.lock_calls, 1);
    assert_eq!(authority.fetch_calls, 1);
    assert!(stored.join("nocter.nct").is_file());
    let canonical_stored = fs::canonicalize(&stored).unwrap();
    assert!(selected.graph().packages().iter().any(|snapshot| {
        snapshot.identity() == &package.package_identity() && snapshot.root() == canonical_stored
    }));
    let manifest = fs::read_to_string(tree.0.join("app/nocter.nct")).unwrap();
    assert!(manifest.contains(&format!("remote: \"git:{COMMIT}\"")));
    assert!(!tree.0.join("app/.nocter/transactions").exists());
}

#[test]
fn invalid_transitive_lock_state_publishes_neither_package_nor_root_lock() {
    let tree = base_tree();
    let original = fs::read(tree.0.join("app/nocter.nct")).unwrap();
    let mut authority = FakeAuthority::new(
        "#name: \"remote\"\n#dependencies: { nested: { git: \"https://example.test/nested.git\", revision: \"main\", }, }\n",
    );

    let error = resolve_package_state(
        tree.request(PackageResolutionPolicy::default()),
        &mut authority,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageStateError::NonRootLockRequired { ref alias, .. } if alias.as_ref() == "nested"
    ));
    assert_eq!(fs::read(tree.0.join("app/nocter.nct")).unwrap(), original);
    assert!(!tree.0.join("app/.nocter/packages").exists());
    assert!(!tree.0.join("app/.nocter").exists());
}

#[test]
fn locked_policy_reaches_resolution_without_creating_transaction_state() {
    let tree = base_tree();
    let mut authority = FakeAuthority::new("#name: \"remote\"\n");

    let error = resolve_package_state(
        tree.request(PackageResolutionPolicy::new(true, false)),
        &mut authority,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageStateError::Resolution(error)
            if matches!(*error, PackageResolutionError::MissingLockLocked { .. })
    ));
    assert_eq!(authority.lock_calls, 0);
    assert_eq!(authority.fetch_calls, 0);
    assert!(!tree.0.join("app/.nocter").exists());
}

#[test]
fn manifest_commit_rejects_a_concurrent_source_change() {
    let tree = TempTree::new();
    tree.source(
        "app/nocter.nct",
        &format!(
            "#dependencies: {{ remote: {{ git: \"https://example.test/remote.git\", revision: \"main\", }}, }}\n#lock: {{ format: 1, dependencies: {{ remote: \"git:{COMMIT}\", }}, }}\n"
        ),
    );
    tree.source("remote/nocter.nct", "#name: \"remote\"\n");
    let app = PackageIdentity::new("app");
    let graph = ResolvedPackageGraph::load(vec![
        ResolvedPackageSpec::new(app.clone(), tree.0.join("app"))
            .with_dependency("remote", PackageIdentity::new("remote")),
        ResolvedPackageSpec::new(PackageIdentity::new("remote"), tree.0.join("remote")),
    ])
    .unwrap();
    let update = graph.manifest_lock_update(&app).unwrap();
    let path = tree.0.join("app/nocter.nct");
    let concurrent = b"#name: \"changed\"\n";
    fs::write(&path, concurrent).unwrap();

    let error = commit_lock_source(&update).unwrap_err();

    assert!(matches!(error, ManifestCommitError::SourceChanged(_)));
    assert_eq!(fs::read(path).unwrap(), concurrent);
}
