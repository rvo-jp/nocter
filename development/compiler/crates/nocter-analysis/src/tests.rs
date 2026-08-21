use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_discovery::{DiscoveryRequest, ToolchainRequest, discover};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay};
use nocter_model::CompilationTarget;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};

use crate::{AnalysisSnapshot, AnalysisStatus, GenerationId};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[test]
fn syntax_failure_retains_generation_overlay_sources_and_diagnostics() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "#name: \"app\"\n");
    tree.source("app/index.nct", "func broken(: void {}\n");
    let source_path = fs::canonicalize(tree.path().join("app/index.nct")).unwrap();
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert(
            source_path.clone(),
            OpenDocument::new(DocumentVersion::new(18), &b"func newer(: void {}\n"[..]),
        )
        .unwrap();
    let overlay = overlay.finish();
    let package = PackageIdentity::new("workspace:app");
    let graph = ResolvedPackageGraph::load_with_source_overlay(
        vec![ResolvedPackageSpec::new(
            package.clone(),
            tree.path().join("app"),
        )],
        overlay,
    )
    .unwrap();
    let root = ModuleIdentity::new(package.clone(), Vec::<&str>::new());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        graph,
        vec![root.clone()],
        ToolchainRequest::new(package, root, Vec::new(), Vec::new()),
    ))
    .unwrap();

    let snapshot = AnalysisSnapshot::compile(GenerationId::new(41), unit);

    assert_eq!(snapshot.generation(), GenerationId::new(41));
    assert_eq!(snapshot.status(), AnalysisStatus::SyntaxFailed);
    assert_eq!(
        snapshot.document_version(&source_path),
        Some(DocumentVersion::new(18))
    );
    assert!(!snapshot.diagnostics().is_empty());
    assert!(!snapshot.syntax_trees().is_empty());
    assert!(snapshot.source_index().is_none());
    assert!(snapshot.target().is_none());
}

#[test]
fn discovery_failure_is_the_generation_result_instead_of_a_stale_success() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "#name: \"app\"\n");
    tree.source("app/index.nct", "func disk(): void { return }\n");
    let source_path = fs::canonicalize(tree.path().join("app/index.nct")).unwrap();
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert(
            source_path.clone(),
            OpenDocument::new(DocumentVersion::new(23), &b"use ./missing\n"[..]),
        )
        .unwrap();
    let package = PackageIdentity::new("workspace:app");
    let graph = ResolvedPackageGraph::load_with_source_overlay(
        vec![ResolvedPackageSpec::new(
            package.clone(),
            tree.path().join("app"),
        )],
        overlay.finish(),
    )
    .unwrap();
    let root = ModuleIdentity::new(package.clone(), Vec::<&str>::new());
    let failure = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        graph,
        vec![root.clone()],
        ToolchainRequest::new(package, root, Vec::new(), Vec::new()),
    ))
    .unwrap_err();

    let snapshot = AnalysisSnapshot::from_discovery_failure(GenerationId::new(42), failure);

    assert_eq!(snapshot.status(), AnalysisStatus::DiscoveryFailed);
    assert_eq!(
        snapshot.document_version(&source_path),
        Some(DocumentVersion::new(23))
    );
    assert!(!snapshot.sources().is_empty());
    assert!(!snapshot.syntax_trees().is_empty());
    assert_eq!(snapshot.diagnostics()[0].code(), "E0263");
    assert!(snapshot.discovery_failure().is_some());
    assert!(snapshot.target().is_none());
}

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("nocter-analysis-{}-{serial}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
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
