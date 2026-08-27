use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_filesystem::{DocumentVersion, OpenDocument, SourceOverlay};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_session::bundled_standard_toolchain;
use nocter_source::ByteOffset;

use crate::{
    AnalysisSnapshot, AnalysisStatus, GenerationId, SemanticCoverage, TypedBodyUnavailability,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[test]
fn syntax_failure_retains_generation_overlay_sources_and_diagnostics() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    );
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
        ToolchainInput::new(package, root, Vec::new(), Vec::new()),
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
    assert!(!snapshot.has_checked_semantics());
}

#[test]
fn discovery_failure_is_the_generation_result_instead_of_a_stale_success() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "func disk(): void { return }\n",
        ),
    );
    let source_path = fs::canonicalize(tree.path().join("app/index.nct")).unwrap();
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_document(
            source_path.clone(),
            OpenDocument::new(
                DocumentVersion::new(23),
                &b"#package: { name: \"app\", version: \"0.0.0\", }\nuse ./missing\n"[..],
            ),
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
        ToolchainInput::new(package, root, Vec::new(), Vec::new()),
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
    assert!(!snapshot.has_checked_semantics());
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
fn repeated_checked_member_queries_are_semantically_identical() {
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
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(51));
    assert_eq!(snapshot.status(), AnalysisStatus::Complete);
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(source_text.find("len").unwrap()).unwrap());
    let accepted_type_count = snapshot
        .semantic_authority()
        .and_then(crate::semantic::SemanticAuthority::complete)
        .expect("checked authority")
        .checked()
        .types()
        .type_count();

    let first = snapshot.semantic_completions(source.id(), offset).unwrap();
    let second = snapshot.semantic_completions(source.id(), offset).unwrap();

    assert_eq!(first, second);
    assert!(first.iter().any(|completion| completion.label() == "len"));
    assert_eq!(
        snapshot
            .semantic_authority()
            .and_then(crate::semantic::SemanticAuthority::complete)
            .expect("checked authority")
            .checked()
            .types()
            .type_count(),
        accepted_type_count
    );
}

#[test]
fn repeated_recovery_member_queries_are_semantically_identical() {
    let tree = TempTree::new();
    let source_text = concat!(
        "func inspect(value: i32): void {\n",
        "    value.missing()\n",
        "    return\n",
        "}\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(52));
    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    let recovery = snapshot
        .semantic_authority()
        .and_then(|authority| authority.body_analysis())
        .expect("typed body recovery");
    assert!(matches!(
        recovery
            .interruptions()
            .next()
            .map(nocter_checking::TypedBodyInterruption::kind),
        Some(nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. })
    ));
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(source_text.find("missing").unwrap()).unwrap());

    let first = snapshot.semantic_completions(source.id(), offset).unwrap();
    let second = snapshot.semantic_completions(source.id(), offset).unwrap();

    assert_eq!(first, second);
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
fn named_builtin_uses_present_and_navigate_through_the_selected_declaration() {
    let tree = TempTree::new();
    let source_text = "func identity(value: i32): i32 { value }\n";
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(49));

    assert_eq!(snapshot.status(), AnalysisStatus::Complete);
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(source_text.find("i32").unwrap()).unwrap());
    let subject = snapshot
        .semantic_subject(source.id(), offset)
        .unwrap()
        .expect("builtin type use has no semantic subject");

    assert_eq!(subject.presentation().code(), "primitive type i32");
    let definitions = snapshot.semantic_definition(source.id(), offset);
    assert_eq!(definitions.len(), 1);
    let definition_source = snapshot
        .sources()
        .get(definitions[0].source())
        .expect("builtin definition source is absent");
    assert!(
        definition_source
            .name()
            .as_str()
            .ends_with("/std/num/index.nct")
    );
}

#[test]
fn declaration_diagnostics_are_complete_while_safe_body_semantics_remain_available() {
    let tree = TempTree::new();
    let source_text = concat!(
        "primitive func first(): usize\n",
        "primitive func second(): usize\n",
        "func inspect(value: i32): i32 {\n",
        "    let local = value\n",
        "    return local\n",
        "}\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(50));

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(
        snapshot
            .diagnostics()
            .iter()
            .map(nocter_diagnostics::SourceDiagnostic::code)
            .collect::<Vec<_>>(),
        ["E0208", "E0208"]
    );
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let local = source_text.rfind("local").unwrap();
    let subject = snapshot
        .semantic_subject(source.id(), ByteOffset::new(u32::try_from(local).unwrap()))
        .unwrap()
        .expect("safe rejected graph lost local body semantics");
    assert_eq!(subject.presentation().code(), "let local: i32");
}

#[test]
fn declaration_failure_retains_the_diagnostics_of_rejected_body_evidence() {
    let tree = TempTree::new();
    let source_text = concat!(
        "primitive func rejected_primitive(): usize\n",
        "func invalid(input: i32?): i32 { input? }\n",
    );
    let (source_path, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(54));

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    let codes = snapshot
        .diagnostics()
        .iter()
        .map(nocter_diagnostics::SourceDiagnostic::code)
        .collect::<Vec<_>>();
    assert_eq!(codes, ["E0208", "E0392"]);
    let recovery = snapshot
        .semantic_authority()
        .and_then(|authority| authority.body_analysis())
        .expect("body evidence beneath declaration rejection");
    assert_eq!(recovery.rejection_diagnostics().count(), 1);
    assert!(
        recovery
            .body_evidence_iter()
            .any(|(_, evidence)| matches!(evidence, nocter_checking::BodyEvidence::Rejected(_)))
    );
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str() == source_path.to_str().unwrap())
        .unwrap();
    let highlights = snapshot.semantic_highlights(source.id()).unwrap();
    let SemanticCoverage::Partial(gaps) = highlights.coverage() else {
        panic!("rejected body was reported as complete coverage")
    };
    assert!(
        gaps.iter()
            .any(|gap| gap.reason() == TypedBodyUnavailability::BodyRejected)
    );
}

#[test]
fn name_recovery_retains_every_rejected_body_diagnostic() {
    let tree = TempTree::new();
    let source_text = concat!(
        "func first(): void { unknown_first\nreturn }\n",
        "func second(): void { unknown_second\nreturn }\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(55));

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(
        snapshot
            .diagnostics()
            .iter()
            .map(nocter_diagnostics::SourceDiagnostic::code)
            .collect::<Vec<_>>(),
        ["E0340", "E0340"]
    );
    let recovery = snapshot
        .retained_semantic()
        .and_then(nocter_session::SemanticAnalysis::names)
        .expect("name evidence");
    assert_eq!(recovery.body_names().rejection_diagnostics().count(), 2);
    assert_eq!(
        recovery
            .body_names()
            .evidence_iter()
            .filter(|(_, evidence)| matches!(
                evidence,
                nocter_checking::BodyNameEvidence::Rejected(_)
            ))
            .count(),
        2
    );
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let highlights = snapshot.semantic_highlights(source.id()).unwrap();
    let SemanticCoverage::Partial(gaps) = highlights.coverage() else {
        panic!("name rejection was reported as complete coverage")
    };
    assert!(
        gaps.iter()
            .any(|gap| gap.reason() == TypedBodyUnavailability::NamesRejected)
    );
}

#[test]
fn quarantined_operation_shapes_cannot_block_independent_body_semantics() {
    for (generation, rejected, expected_code) in [
        (
            51,
            "instance str {\n    method &self.invalid(): i32 { return 0 }\n}\n",
            "E0201",
        ),
        (
            52,
            "interface Pair {\n    pub type First\n    pub type Second\n}\nstruct Box {}\ninstance Box {\n    impl Pair { .First = i32 }\n}\n",
            "E0211",
        ),
    ] {
        let tree = TempTree::new();
        let source_text = format!(
            "{rejected}func inspect(value: i32): i32 {{\n    let retained = value\n    return retained\n}}\n"
        );
        let (_, snapshot) = bundled_snapshot(&tree, &source_text, GenerationId::new(generation));

        assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
        let codes = snapshot
            .diagnostics()
            .iter()
            .map(nocter_diagnostics::SourceDiagnostic::code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&expected_code), "{codes:?}");
        let source = snapshot
            .sources()
            .iter()
            .find(|source| source.name().as_str().ends_with("app.nct"))
            .unwrap();
        let retained = source_text.rfind("retained").unwrap();
        let subject = snapshot
            .semantic_subject(
                source.id(),
                ByteOffset::new(u32::try_from(retained).unwrap()),
            )
            .unwrap()
            .expect("quarantined operation blocked an independent body");
        assert_eq!(subject.presentation().code(), "let retained: i32");
    }
}

#[test]
fn program_fact_rejection_stops_at_the_declaration_capability() {
    let tree = TempTree::new();
    let source_text = concat!(
        "enum Empty {}\n",
        "func inspect(value: i32): i32 {\n",
        "    let unavailable = value\n",
        "    return unavailable\n",
        "}\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(53));

    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(snapshot.diagnostics()[0].code(), "E0200");
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let unavailable = source_text.rfind("unavailable").unwrap();
    assert!(
        snapshot
            .semantic_subject(
                source.id(),
                ByteOffset::new(u32::try_from(unavailable).unwrap()),
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn syntax_and_declaration_failure_share_the_current_declaration_authority() {
    let tree = TempTree::new();
    let source_text = concat!(
        "pub interface Readable { pub method &self.read(): i32 }\n",
        "struct Value {}\n",
        "instance Value { impl Readable }\n",
        "func inspect(value: &Value): void {\n",
        "    value.\n",
        "    return\n",
        "}\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(47));

    assert_eq!(snapshot.status(), AnalysisStatus::SyntaxFailed);
    assert!(!snapshot.has_checked_semantics());
    let interface_implementation_diagnostic = snapshot
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "E0350")
        .expect("independent interface_implementation diagnostic");
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(source_text.find("Readable").unwrap()).unwrap());
    let subject = snapshot
        .semantic_subject(source.id(), offset)
        .unwrap()
        .expect("declaration subject");
    assert_eq!(subject.presentation().code(), "pub interface Readable");
    assert!(
        snapshot
            .semantic_rename(source.id(), offset, "ReadableValue")
            .unwrap()
            .is_none(),
        "rename must require complete semantic occurrence coverage"
    );
    let actions = snapshot
        .semantic_code_actions(
            source.id(),
            interface_implementation_diagnostic.primary().span().range(),
        )
        .unwrap();
    assert!(!actions.is_empty());
}

#[test]
fn rename_validation_rejects_a_candidate_without_checked_semantics() {
    let tree = TempTree::new();
    let original_text = concat!(
        "func helper(): i32 { return 1 }\n",
        "func main(): i32 { return helper() }\n",
    );
    let (_, original) = bundled_snapshot(&tree, original_text, GenerationId::new(49));
    let source = original
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(original_text.find("helper").unwrap()).unwrap());
    let plan = original
        .semantic_rename(source.id(), offset, "calculate")
        .unwrap()
        .expect("rename plan");

    let candidate_text = concat!(
        "func calculate(): i32 { return 1 }\n",
        "func main(): i32 { return calculate() }\n",
        "func broken(): void {\n",
        "    unknown\n",
        "    return\n",
        "}\n",
    );
    let (_, candidate) = bundled_snapshot(&tree, candidate_text, GenerationId::new(50));
    assert_eq!(candidate.status(), AnalysisStatus::CompilationFailed);
    assert!(!candidate.has_checked_semantics());
    assert!(!original.validates_rename_candidate(&plan, &candidate));
}

#[test]
fn syntax_and_name_failure_share_the_current_name_authority() {
    let tree = TempTree::new();
    let source_text = concat!(
        "struct Text {}\n",
        "func inspect(value: &Text): void {\n",
        "    unknown\n",
        "    value.\n",
        "    return\n",
        "}\n",
    );
    let (_, snapshot) = bundled_snapshot(&tree, source_text, GenerationId::new(48));

    assert_eq!(snapshot.status(), AnalysisStatus::SyntaxFailed);
    assert!(
        snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0340")
    );
    let source = snapshot
        .sources()
        .iter()
        .find(|source| source.name().as_str().ends_with("app.nct"))
        .unwrap();
    let offset = ByteOffset::new(u32::try_from(source_text.find("Text").unwrap()).unwrap());
    let subject = snapshot
        .semantic_subject(source.id(), offset)
        .unwrap()
        .expect("name-stage subject");
    assert_eq!(subject.presentation().code(), "struct Text");
}

#[test]
fn namespace_call_accepts_the_callable_reexport_selected_by_name_resolution() {
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "use ./surface\n",
            "func main(): i32 { return surface.implementation.answer() }\n",
        ),
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
    tree.source(
        "app/index.nct",
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "use ./implementation\n",
            "func main(): i32 { return implementation.answer() }\n",
        ),
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
