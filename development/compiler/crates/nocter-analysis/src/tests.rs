use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveryRequest, ToolchainRequest, discover};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_session::bundled_standard_toolchain;
use nocter_source::ByteOffset;

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
        .insert_document(
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
        .insert_document(
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

#[test]
fn namespace_member_call_projects_the_callable_for_hover_and_navigation() {
    let tree = TempTree::new();
    let source_text = concat!(
        "use std/fs\n",
        "func inspect(path: &str): void! {\n",
        "    let details = fs.metadata(path)?\n",
        "    let _ = details.len()\n",
        "    return\n",
        "}\n",
        "func main(): i32 { return 0 }\n",
    );
    let (source_path, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(43));
    assert_eq!(
        snapshot.status(),
        AnalysisStatus::Complete,
        "namespace fixture diagnostics: {:#?}",
        snapshot.diagnostics()
    );

    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str() == source_path.to_str().unwrap())
        .unwrap();
    let member_offset = source_text.find("metadata").unwrap();
    let offset = ByteOffset::new(u32::try_from(member_offset).unwrap());
    let subject = snapshot
        .semantic_subject(source.id(), offset)
        .unwrap()
        .unwrap();
    assert_eq!(
        subject.presentation().code(),
        "pub func metadata(path: &str): Metadata!"
    );
    assert_eq!(snapshot.semantic_definition(source.id(), offset).len(), 1);
    assert_eq!(
        snapshot.semantic_implementation(source.id(), offset).len(),
        1
    );
}

#[test]
fn missing_namespace_member_is_a_source_diagnostic_not_an_internal_failure() {
    let tree = TempTree::new();
    let (_, snapshot) = bundled_snapshot(
        &tree,
        "use std/fs\nfunc main(): void { fs.missing() }\n",
        GenerationId::new(44),
    );

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(snapshot.diagnostics().len(), 1);
    assert_eq!(snapshot.diagnostics()[0].code(), "E0347");
}

#[test]
fn namespace_call_accepts_the_callable_reexport_selected_by_name_resolution() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "#name: \"app\"\n");
    tree.source(
        "app/index.nct",
        "use ./surface\nfunc main(): i32 { return surface.implementation.answer() }\n",
    );
    tree.source("app/surface/index.nct", "pub use ../implementation\n");
    tree.source(
        "app/implementation/index.nct",
        "pub func answer(): i32 { return 42 }\n",
    );
    let snapshot = declared_bundled_snapshot(&tree, GenerationId::new(45));

    assert_eq!(
        snapshot.status(),
        AnalysisStatus::Complete,
        "re-export fixture diagnostics: {:#?}; failure: {:#?}",
        snapshot.diagnostics(),
        snapshot.compilation_failure()
    );
}

#[test]
fn inaccessible_namespace_member_uses_the_module_visibility_diagnostic() {
    let tree = TempTree::new();
    tree.source("app/nocter.nct", "#name: \"app\"\n");
    tree.source(
        "app/index.nct",
        "use ./implementation\nfunc main(): i32 { return implementation.answer() }\n",
    );
    tree.source(
        "app/implementation/index.nct",
        "func answer(): i32 { return 42 }\n",
    );

    let snapshot = declared_bundled_snapshot(&tree, GenerationId::new(46));

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(snapshot.diagnostics().len(), 1);
    assert_eq!(snapshot.diagnostics()[0].code(), "E0348");
}

fn declared_bundled_snapshot(tree: &TempTree, generation: GenerationId) -> AnalysisSnapshot {
    let package = PackageIdentity::new("workspace:app");
    let standard = PackageIdentity::new("toolchain:std");
    let standard_root =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std")).unwrap();
    let graph = ResolvedPackageGraph::load(vec![
        ResolvedPackageSpec::new(package.clone(), tree.path().join("app"))
            .with_standard_dependency(standard.clone()),
        ResolvedPackageSpec::new(standard.clone(), standard_root)
            .with_standard_dependency(standard.clone()),
    ])
    .unwrap();
    let root = ModuleIdentity::new(package, Vec::<&str>::new());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        graph,
        vec![root],
        bundled_standard_toolchain(&standard),
    ))
    .unwrap();
    AnalysisSnapshot::compile(generation, unit)
}

fn bundled_snapshot(
    tree: &TempTree,
    source_text: &str,
    generation: GenerationId,
) -> (PathBuf, AnalysisSnapshot) {
    tree.source("app.nct", source_text);
    let source_path = fs::canonicalize(tree.path().join("app.nct")).unwrap();
    let standard_root =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std")).unwrap();
    let standard = PackageIdentity::new("toolchain:std");
    let graph = ResolvedPackageGraph::load(vec![
        ResolvedPackageSpec::new(standard.clone(), &standard_root)
            .with_standard_dependency(standard.clone()),
    ])
    .unwrap();
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        &source_path,
        graph,
        bundled_standard_toolchain(&standard),
    ))
    .unwrap();
    (source_path, AnalysisSnapshot::compile(generation, unit))
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
