use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_analysis::AnalysisStatus;
use nocter_filesystem::DocumentVersion;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::StandardPackage;
use nocter_workspace_revision::{DocumentChange, WorkspaceDocuments, WorkspaceSourceRevision};

use super::*;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

enum DocumentWorkspaceChange {
    Accepted(WorkspaceSourceRevision),
    IgnoredStale,
}

#[derive(Default)]
struct DocumentWorkspace {
    documents: WorkspaceDocuments,
}

impl DocumentWorkspace {
    fn new() -> Self {
        Self::default()
    }

    fn open(
        &mut self,
        path: &Path,
        version: i32,
        text: &str,
    ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
        let path = canonical_document_path(path)?;
        let revision = self.documents.open(
            path.clone(),
            DocumentVersion::new(version),
            Arc::<[u8]>::from(text.as_bytes()),
        )?;
        Ok(revision)
    }

    fn change(
        &mut self,
        path: &Path,
        version: i32,
        text: &str,
    ) -> Result<DocumentWorkspaceChange, Box<dyn std::error::Error>> {
        let path = canonical_document_path(path)?;
        Ok(
            match self.documents.change(
                &path,
                DocumentVersion::new(version),
                Arc::<[u8]>::from(text.as_bytes()),
            )? {
                DocumentChange::Accepted(revision) => DocumentWorkspaceChange::Accepted(revision),
                DocumentChange::IgnoredStale { .. } => DocumentWorkspaceChange::IgnoredStale,
            },
        )
    }

    fn close(
        &mut self,
        path: &Path,
    ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
        let path = canonical_document_path(path)?;
        let revision = self.documents.close(&path)?;
        Ok(revision)
    }

    fn refresh(
        &mut self,
        sources: &[&Path],
    ) -> Result<WorkspaceSourceRevision, Box<dyn std::error::Error>> {
        let paths = sources
            .iter()
            .map(|path| canonical_document_path(path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.documents.refresh(paths)?)
    }
}

fn canonical_document_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = fs::canonicalize(path.parent().ok_or("document has no parent")?)?;
    Ok(parent.join(path.file_name().ok_or("document has no file name")?))
}

#[test]
fn package_generation_uses_overlay_bytes_and_reaches_compiler_analysis() {
    let temporary = TemporaryDirectory::new();
    fs::write(
        temporary.path().join("index.nct"),
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "pub func answer(): i32 { return 42 }\n",
        ),
    )
    .unwrap();
    let source = temporary.path().join("index.nct");
    let configuration = configuration(temporary.path());
    let mut documents = DocumentWorkspace::new();
    let accepted = documents
        .open(
            &source,
            7,
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "pub func answer(): i32 { return }\n",
            ),
        )
        .unwrap();
    let canonical_source = accepted.primary_document().to_path_buf();
    let mut analyses = WorkspaceAnalyses::new(configuration);

    let analyzed = analyses.analyze(accepted).unwrap();

    assert!(matches!(
        analyzed.primary().scope(),
        Some(AnalysisScope::Package(_))
    ));
    let snapshot = analyzed
        .primary()
        .snapshot()
        .expect("discovery reaches analysis");
    assert_eq!(snapshot.status(), AnalysisStatus::CompilationFailed);
    assert_eq!(
        snapshot
            .source_overlay()
            .document(&canonical_source)
            .unwrap()
            .bytes(),
        concat!(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            "pub func answer(): i32 { return }\n",
        )
        .as_bytes()
    );
    assert_eq!(
        analyses
            .latest(analyzed.primary().scope().unwrap())
            .unwrap()
            .generation(),
        analyzed.primary().generation()
    );
}

#[test]
fn incomplete_syntax_analysis_is_a_reusable_exact_current_query() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let source = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func inspect(value: &str): void {\n",
        "    value.\n",
        "    return\n",
        "}\n",
    );
    fs::write(&root, source).unwrap();
    let canonical_root = fs::canonicalize(&root).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let first = analyses
        .analyze(documents.open(&root, 1, source).unwrap())
        .unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::SyntaxFailed
    );
    assert_eq!(analyses.incomplete_analysis_counts().0, 1);

    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, source).unwrap()
    else {
        panic!("newer document version is accepted");
    };
    let second = analyses.analyze(revision).unwrap();
    let second_snapshot = second.primary().snapshot().unwrap();
    assert_eq!(second_snapshot.status(), AnalysisStatus::SyntaxFailed);
    assert_eq!(
        second_snapshot.document_version(&canonical_root),
        Some(nocter_filesystem::DocumentVersion::new(2)),
        "semantic reuse must retain the current editor envelope",
    );
    let counts = analyses.incomplete_analysis_counts();
    assert_eq!(counts.0, 1);
    assert!(counts.1 > 0);
}

#[test]
fn unchanged_source_text_reuses_parsing_across_workspace_revisions() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let helper = temporary.path().join("helper.nct");
    let root_text = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func answer(): i32 { return helper() }\n",
    );
    let changed_root_text = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func answer(): i32 { return helper() + 1 }\n",
    );
    let helper_text = "func helper(): i32 { return 41 }\n";
    let changed_helper_text = "func helper(): i32 { return 42 }\n";
    fs::write(&root, root_text).unwrap();
    fs::write(&helper, helper_text).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    analyses
        .analyze(documents.open(&root, 1, root_text).unwrap())
        .unwrap();
    let after_initial = analyses.source_parse_counts();
    let surfaces_after_initial = analyses.declaration_surface_counts();
    let declarations_after_initial = analyses.declaration_query_counts();
    assert!(after_initial.0 > 0);

    let DocumentWorkspaceChange::Accepted(root_revision) =
        documents.change(&root, 2, changed_root_text).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    analyses.analyze(root_revision).unwrap();
    let after_root_change = analyses.source_parse_counts();
    let surfaces_after_root_change = analyses.declaration_surface_counts();
    let declarations_after_root_change = analyses.declaration_query_counts();
    assert_eq!(after_root_change.0, after_initial.0 + 1);
    assert!(after_root_change.1 > after_initial.1);
    assert_eq!(surfaces_after_root_change.0, surfaces_after_initial.0 + 1);
    assert_eq!(surfaces_after_root_change.1, surfaces_after_initial.1);
    assert!(surfaces_after_root_change.2 > surfaces_after_initial.2);
    assert_eq!(
        declarations_after_root_change.0,
        declarations_after_initial.0
    );
    assert!(declarations_after_root_change.1 > declarations_after_initial.1);

    analyses
        .analyze(documents.open(&helper, 1, helper_text).unwrap())
        .unwrap();
    let before_helper_change = analyses.source_parse_counts();
    let surfaces_before_helper_change = analyses.declaration_surface_counts();
    let declarations_before_helper_change = analyses.declaration_query_counts();
    let DocumentWorkspaceChange::Accepted(helper_revision) =
        documents.change(&helper, 2, changed_helper_text).unwrap()
    else {
        panic!("newer helper text is accepted");
    };
    let warm = analyses.analyze(helper_revision).unwrap();
    let after_helper_change = analyses.source_parse_counts();
    let surfaces_after_helper_change = analyses.declaration_surface_counts();
    let declarations_after_helper_change = analyses.declaration_query_counts();
    assert_eq!(after_helper_change.0, before_helper_change.0 + 1);
    assert_eq!(
        surfaces_after_helper_change.0,
        surfaces_before_helper_change.0 + 1
    );
    assert_eq!(
        surfaces_after_helper_change.1,
        surfaces_before_helper_change.1
    );
    assert_eq!(
        declarations_after_helper_change.0,
        declarations_before_helper_change.0
    );
    assert!(declarations_after_helper_change.1 > declarations_before_helper_change.1);

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed_root_text).unwrap())
        .unwrap();
    let fresh = fresh_analyses
        .analyze(
            fresh_documents
                .open(&helper, 1, changed_helper_text)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn rejected_declarations_depend_on_exact_current_source() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "struct Duplicate {}\n",
        "struct Duplicate {}\n",
        "func body(): i32 { return 1 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "struct Duplicate {}\n",
        "struct Duplicate {}\n",
        "func body(): i32 { return 2 }\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let initial = analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    assert_eq!(
        initial.primary().snapshot().unwrap().status(),
        AnalysisStatus::CompilationFailed
    );
    let before = analyses.declaration_query_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();
    let after = analyses.declaration_query_counts();

    assert_eq!(after.0, before.0 + 1);
    assert!(!warm.primary().snapshot().unwrap().diagnostics().is_empty());

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[derive(Debug, Eq, PartialEq)]
struct AnalysisSignature {
    status: AnalysisStatus,
    diagnostics: Vec<DiagnosticSignature>,
    sources: Vec<(Box<str>, Box<str>)>,
}

/// Observable diagnostic identity used when comparing independent compiler owners.
///
/// `SourceId` and `NodeId` deliberately contain process-local integrity identities. Comparing
/// the raw diagnostic envelopes would therefore conflate externally equal diagnostics with
/// internal authority aliasing, exactly the condition the source model is designed to reject.
#[derive(Debug, Eq, PartialEq)]
struct DiagnosticSignature {
    code: Box<str>,
    message: Box<str>,
    primary: DiagnosticOriginSignature,
    notes: Vec<(Box<str>, DiagnosticOriginSignature)>,
    help: Option<Box<str>>,
    repair: Option<nocter_diagnostics::DiagnosticRepair>,
}

#[derive(Debug, Eq, PartialEq)]
struct DiagnosticOriginSignature {
    source: Box<str>,
    range: nocter_source::TextRange,
}

fn assert_query_reused(before: (u64, u64), after: (u64, u64)) {
    assert_eq!(after.0, before.0);
    assert!(after.1 > before.1);
}

#[test]
fn body_only_edit_reuses_program_wide_preparation() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func answer(): i32 { return 1 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func answer(): i32 { return 2 }\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    let before = analyses.program_preparation_counts();
    let materialization_before = analyses.program_materialization_counts();
    let relations_before = analyses.program_relation_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();

    assert_query_reused(before, analyses.program_preparation_counts());
    let materialization_after = analyses.program_materialization_counts();
    assert_eq!(materialization_after.0, materialization_before.0 + 1);
    assert_query_reused(relations_before, analyses.program_relation_counts());

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn body_edit_resolves_only_the_changed_body_and_rebinds_unchanged_siblings() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func first(): i32 { return 1 }\n",
        "func second(): i32 { return 2 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func first(): i32 {\n",
        "    let value = 1\n",
        "    return value\n",
        "}\n",
        "func second(): i32 { return 2 }\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    let before = analyses.body_name_query_counts();
    let typed_before = analyses.typed_body_query_counts();
    let relations_before = analyses.program_relation_counts();
    let finalization_before = analyses.program_finalization_counts();
    let analysis_before = analyses.program_analysis_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();
    let after = analyses.body_name_query_counts();
    assert_eq!(after.0, before.0 + 1);
    assert!(after.1 > before.1);
    let typed_after = analyses.typed_body_query_counts();
    assert_eq!(typed_after.0, typed_before.0 + 1);
    assert!(typed_after.1 > typed_before.1);
    let relations_after = analyses.program_relation_counts();
    assert_eq!(relations_after.0, relations_before.0 + 1);
    let finalization_after = analyses.program_finalization_counts();
    assert_eq!(finalization_after.0, finalization_before.0 + 1);
    let analysis_after = analyses.program_analysis_counts();
    assert_eq!(analysis_after.0, analysis_before.0 + 1);

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn reused_relation_failure_projects_through_the_current_body_generation() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func inspect(value: &i32): void { return }\n",
        "func invalid(): void {\n",
        "    let ignored = 1\n",
        "    var value = 1\n",
        "    let read = &value\n",
        "    let write = &+value\n",
        "    inspect(read)\n",
        "    return\n",
        "}\n",
    );
    let changed = original.replace("let ignored = 1", "let ignored = 100000");
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let first = analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::CompilationFailed
    );
    let relations_before = analyses.program_relation_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, &changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();

    assert_query_reused(relations_before, analyses.program_relation_counts());
    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, &changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn authored_body_rejection_reuses_unchanged_typed_siblings() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func rejected(): i32 { return true }\n",
        "func unchanged(): i32 { return 2 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func rejected(): i32 {\n",
        "    let value = true\n",
        "    return value\n",
        "}\n",
        "func unchanged(): i32 { return 2 }\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let first = analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::CompilationFailed
    );
    let before = analyses.typed_body_query_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();
    let after = analyses.typed_body_query_counts();
    assert_eq!(after.0, before.0 + 1);
    assert!(after.1 > before.1);

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn authored_name_rejection_reuses_unchanged_lexical_siblings() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func rejected(): void {\n",
        "    unknown\n",
        "    return\n",
        "}\n",
        "func unchanged(): i32 { return 2 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func rejected(): void {\n",
        "    let before = 1\n",
        "    unknown\n",
        "    return\n",
        "}\n",
        "func unchanged(): i32 { return 2 }\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let first = analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::CompilationFailed
    );
    let before = analyses.body_name_query_counts();
    let finalization_before = analyses.program_finalization_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();
    let after = analyses.body_name_query_counts();
    let finalization_after = analyses.program_finalization_counts();
    assert_eq!(after.0, before.0 + 1);
    assert!(after.1 > before.1);
    assert_eq!(finalization_after.0, finalization_before.0 + 1);

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

#[test]
fn authored_preparation_rejection_stays_exact_current_and_matches_fresh_analysis() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let original = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "pub interface Readable {\n",
        "    pub method &self.read(): i32\n",
        "}\n",
        "struct Value {}\n",
        "instance Value { impl Readable }\n",
        "func helper(): i32 { return 1 }\n",
    );
    let changed = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "pub interface Readable {\n",
        "    pub method &self.read(): i32\n",
        "}\n",
        "struct Value {}\n",
        "instance Value { impl Readable }\n",
        "func helper(): i32 {\n",
        "    return 2\n",
        "}\n",
    );
    fs::write(&root, original).unwrap();
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));

    let first = analyses
        .analyze(documents.open(&root, 1, original).unwrap())
        .unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::CompilationFailed
    );
    let before = analyses.program_preparation_counts();
    let DocumentWorkspaceChange::Accepted(revision) = documents.change(&root, 2, changed).unwrap()
    else {
        panic!("newer root text is accepted");
    };
    let warm = analyses.analyze(revision).unwrap();
    let after = analyses.program_preparation_counts();
    assert_eq!(after.0, before.0 + 1);

    let mut fresh_documents = DocumentWorkspace::new();
    let mut fresh_analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let fresh = fresh_analyses
        .analyze(fresh_documents.open(&root, 1, changed).unwrap())
        .unwrap();
    assert_eq!(
        analysis_signature(warm.primary()),
        analysis_signature(fresh.primary())
    );
}

fn analysis_signature(generation: &WorkspaceAnalysisGeneration) -> AnalysisSignature {
    let snapshot = generation.snapshot().expect("analysis reached discovery");
    let sources = snapshot
        .sources()
        .iter()
        .map(|source| (source.name().as_str().into(), source.text().into()))
        .collect();
    let diagnostics = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| DiagnosticSignature {
            code: diagnostic.code().into(),
            message: diagnostic.message().into(),
            primary: diagnostic_origin_signature(snapshot.sources(), diagnostic.primary()),
            notes: diagnostic
                .notes()
                .iter()
                .map(|note| {
                    (
                        note.message().into(),
                        diagnostic_origin_signature(snapshot.sources(), note.origin()),
                    )
                })
                .collect(),
            help: diagnostic.help().map(Into::into),
            repair: diagnostic.repair().cloned(),
        })
        .collect();
    AnalysisSignature {
        status: snapshot.status(),
        diagnostics,
        sources,
    }
}

fn diagnostic_origin_signature(
    sources: &nocter_source::SourceMap,
    origin: nocter_diagnostics::DiagnosticOrigin,
) -> DiagnosticOriginSignature {
    let source = sources
        .get(origin.source())
        .expect("a published diagnostic source belongs to its analysis snapshot");
    DiagnosticOriginSignature {
        source: source.name().as_str().into(),
        range: origin.span().range(),
    }
}

#[test]
fn package_generation_retains_every_open_module_root_without_a_representative_source() {
    let temporary = TemporaryDirectory::new();
    fs::write(
        temporary.path().join("index.nct"),
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    )
    .unwrap();
    let first_directory = temporary.path().join("first");
    let second_directory = temporary.path().join("second");
    fs::create_dir(&first_directory).unwrap();
    fs::create_dir(&second_directory).unwrap();
    let first = first_directory.join("index.nct");
    let second = second_directory.join("index.nct");
    let first_text = "pub func first(): i32 { return 1 }\n";
    let second_text = "pub func second(): i32 { return 2 }\n";
    fs::write(&first, first_text).unwrap();
    fs::write(&second, second_text).unwrap();

    let configuration = configuration(temporary.path());
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration.clone());
    let first_revision = documents.open(&first, 1, first_text).unwrap();
    let canonical_first = first_revision.primary_document().to_path_buf();
    analyses.analyze(first_revision).unwrap();
    let second_revision = documents.open(&second, 1, second_text).unwrap();
    let canonical_second = second_revision.primary_document().to_path_buf();
    let generation = analyses.analyze(second_revision).unwrap();
    let snapshot = generation
        .primary()
        .snapshot()
        .expect("package analysis snapshot");

    assert!(
        snapshot
            .sources()
            .find_by_name(canonical_first.to_str().unwrap())
            .is_some()
    );
    assert!(
        snapshot
            .sources()
            .find_by_name(canonical_second.to_str().unwrap())
            .is_some()
    );
    assert_eq!(
        analyses
            .latest_for_document(&canonical_first)
            .unwrap()
            .unwrap()
            .generation(),
        generation.primary().generation()
    );

    let mut reverse_documents = DocumentWorkspace::new();
    let mut reverse_analyses = WorkspaceAnalyses::new(configuration);
    reverse_analyses
        .analyze(reverse_documents.open(&second, 1, second_text).unwrap())
        .unwrap();
    let reverse_generation = reverse_analyses
        .analyze(reverse_documents.open(&first, 1, first_text).unwrap())
        .unwrap();
    let reverse_sources = reverse_generation.primary().snapshot().unwrap().sources();
    assert!(
        reverse_sources
            .find_by_name(canonical_first.to_str().unwrap())
            .is_some()
    );
    assert!(
        reverse_sources
            .find_by_name(canonical_second.to_str().unwrap())
            .is_some()
    );
}

#[test]
fn watched_source_change_does_not_add_a_closed_module_to_package_demand() {
    let temporary = TemporaryDirectory::new();
    let root = temporary.path().join("index.nct");
    let root_text = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func main(): i32 { return 0 }\n",
    );
    fs::write(&root, root_text).unwrap();
    let closed_directory = temporary.path().join("closed");
    fs::create_dir(&closed_directory).unwrap();
    let closed = closed_directory.join("index.nct");
    fs::write(&closed, "pub func closed(): i32 { return 1 }\n").unwrap();

    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    let opened = documents.open(&root, 1, root_text).unwrap();
    let first = analyses.analyze(opened).unwrap();
    let canonical_closed = fs::canonicalize(&closed).unwrap();
    assert!(
        first
            .primary()
            .snapshot()
            .unwrap()
            .sources()
            .find_by_name(canonical_closed.to_str().unwrap())
            .is_none()
    );

    let refreshed = documents.refresh(&[&closed]).unwrap();
    let generation = analyses.analyze(refreshed).unwrap();

    assert!(
        generation
            .primary()
            .snapshot()
            .unwrap()
            .sources()
            .find_by_name(canonical_closed.to_str().unwrap())
            .is_none(),
        "a change invalidates current demand but does not become semantic demand itself"
    );
}

#[test]
fn source_without_a_bounded_package_root_uses_single_file_mode() {
    let temporary = TemporaryDirectory::new();
    let source = temporary.path().join("standalone.nct");
    let configuration = configuration(temporary.path());
    let mut documents = DocumentWorkspace::new();
    let accepted = documents
        .open(&source, 1, "func main(): void { return }\n")
        .unwrap();
    let canonical_source = accepted.primary_document().to_path_buf();

    let analyzed = WorkspaceAnalyses::new(configuration)
        .analyze(accepted)
        .unwrap();

    assert_eq!(
        analyzed.primary().scope(),
        Some(&AnalysisScope::SingleFile(canonical_source))
    );
    assert_eq!(
        analyzed.primary().snapshot().unwrap().status(),
        AnalysisStatus::Complete,
        "{:?}",
        analyzed.primary().snapshot().unwrap().diagnostics()
    );
}

#[test]
fn toolchain_standard_inside_workspace_keeps_its_selected_identity() {
    let standard_root = standard_root();
    let workspace_root = standard_root.parent().unwrap();
    let source = standard_root.join("error/index.nct");
    let text = fs::read_to_string(&source).unwrap();
    let configuration = configuration_with_standard(workspace_root, &standard_root);
    let mut documents = DocumentWorkspace::new();
    let accepted = documents.open(&source, 1, &text).unwrap();

    let analyzed = WorkspaceAnalyses::new(configuration)
        .analyze(accepted)
        .unwrap();

    assert_eq!(
        analyzed.primary().scope(),
        Some(&AnalysisScope::ToolchainStandard(standard_root))
    );
    assert!(analyzed.primary().preparation_failure().is_none());
    assert_eq!(
        analyzed.primary().snapshot().unwrap().status(),
        AnalysisStatus::Complete,
        "diagnostics={:?}",
        analyzed.primary().snapshot().unwrap().diagnostics()
    );
}

#[test]
fn toolchain_standard_outside_workspace_shares_one_complete_overlay_snapshot() {
    let temporary = TemporaryDirectory::new();
    let standard_root = standard_root();
    let contract = standard_root.join("error/index.nct");
    let implementation = standard_root.join("error/construction.nct");
    let contract_text = fs::read_to_string(&contract).unwrap();
    let implementation_text = format!(
        "{}\n// Accepted editor overlay.\n",
        fs::read_to_string(&implementation).unwrap()
    );
    let configuration = configuration_with_standard(temporary.path(), &standard_root);
    let mut documents = DocumentWorkspace::new();
    let contract_generation = documents.open(&contract, 1, &contract_text).unwrap();
    let canonical_contract = contract_generation.primary_document().to_path_buf();
    let mut analyses = WorkspaceAnalyses::new(configuration);
    let first = analyses.analyze(contract_generation).unwrap();
    assert_eq!(
        first.primary().snapshot().unwrap().status(),
        AnalysisStatus::Complete,
        "diagnostics={:?}",
        first.primary().snapshot().unwrap().diagnostics()
    );

    let implementation_generation = documents
        .open(&implementation, 3, &implementation_text)
        .unwrap();
    let canonical_implementation = implementation_generation.primary_document().to_path_buf();
    let second = analyses.analyze(implementation_generation).unwrap();

    assert_eq!(first.primary().scope(), second.primary().scope());
    assert_eq!(
        second.primary().scope(),
        Some(&AnalysisScope::ToolchainStandard(standard_root))
    );
    assert_eq!(
        second.primary().snapshot().unwrap().status(),
        AnalysisStatus::Complete
    );
    assert_eq!(
        second
            .primary()
            .source_overlay()
            .document(&canonical_implementation)
            .unwrap()
            .bytes(),
        implementation_text.as_bytes()
    );
    assert_eq!(
        analyses
            .latest_for_document(&canonical_contract)
            .unwrap()
            .unwrap()
            .generation(),
        second.primary().generation()
    );
}

#[test]
fn a_shared_dependency_source_never_selects_a_package_context_by_sort_order() {
    let temporary = TemporaryDirectory::new();
    let standard_root = standard_root();
    let configuration = configuration_with_standard(temporary.path(), &standard_root);
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration);

    for (directory, name) in [("first", "first"), ("second", "second")] {
        let root = temporary.path().join(directory);
        fs::create_dir(&root).unwrap();
        let source = root.join("index.nct");
        let text = format!("#package: {{ name: \"{name}\", version: \"0.0.0\", }}\nuse std/fs\n");
        fs::write(&source, &text).unwrap();
        analyses
            .analyze(documents.open(&source, 1, &text).unwrap())
            .unwrap();
    }

    let dependency_source = standard_root.join("fs/index.nct");
    let ambiguity = analyses
        .latest_for_document(&dependency_source)
        .expect_err("a dependency source shared by two packages has no implicit authority");

    assert_eq!(ambiguity.document(), dependency_source);
    assert_eq!(ambiguity.candidates().len(), 2);
    assert!(
        ambiguity
            .candidates()
            .iter()
            .all(|scope| matches!(scope, AnalysisScope::Package(_)))
    );
}

#[test]
fn package_topology_change_reassigns_every_known_document_atomically() {
    let temporary = TemporaryDirectory::new();
    let index = temporary.path().join("index.nct");
    let helper = temporary.path().join("helper.nct");
    let package_text = concat!(
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
        "func main(): i32 { return 0 }\n",
    );
    let helper_text = "func helper(): i32 { return 1 }\n";
    fs::write(&index, package_text).unwrap();
    fs::write(&helper, helper_text).unwrap();
    let configuration = configuration(temporary.path());
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration);

    let index_generation = documents.open(&index, 1, package_text).unwrap();
    let canonical_index = index_generation.primary_document().to_path_buf();
    let first = analyses.analyze(index_generation).unwrap();
    let package_scope = first.primary().scope().unwrap().clone();

    let helper_generation = documents.open(&helper, 1, helper_text).unwrap();
    let canonical_helper = helper_generation.primary_document().to_path_buf();
    let second = analyses.analyze(helper_generation).unwrap();
    assert_eq!(second.primary().scope(), Some(&package_scope));

    let DocumentWorkspaceChange::Accepted(changed) = documents
        .change(&index, 2, "func main(): i32 { return 0 }\n")
        .unwrap()
    else {
        panic!("current topology change was ignored")
    };
    let batch = analyses.analyze(changed).unwrap();

    assert_eq!(
        batch.primary().scope(),
        Some(&AnalysisScope::SingleFile(canonical_index.clone()))
    );
    assert!(
        batch
            .current_generations()
            .all(|generation| generation.scope() != Some(&package_scope))
    );
    assert!(batch.updated_generations().any(|generation| {
        generation.scope() == Some(&AnalysisScope::SingleFile(canonical_helper.clone()))
    }));
    assert_eq!(
        analyses
            .latest_for_document(&canonical_index)
            .unwrap()
            .and_then(WorkspaceAnalysisGeneration::scope),
        Some(&AnalysisScope::SingleFile(canonical_index))
    );
    assert_eq!(
        analyses
            .latest_for_document(&canonical_helper)
            .unwrap()
            .and_then(WorkspaceAnalysisGeneration::scope),
        Some(&AnalysisScope::SingleFile(canonical_helper))
    );
}

#[test]
fn closing_a_document_removes_it_from_the_current_workspace_domain() {
    let temporary = TemporaryDirectory::new();
    let source = temporary.path().join("standalone.nct");
    fs::write(&source, "func main(): void { return }\n").unwrap();
    let configuration = configuration(temporary.path());
    let mut documents = DocumentWorkspace::new();
    let mut analyses = WorkspaceAnalyses::new(configuration);
    let opened = documents
        .open(&source, 1, "func main(): void { return }\n")
        .unwrap();
    let canonical = opened.primary_document().to_path_buf();
    let active = analyses.analyze(opened).unwrap();
    let active_scope = active.primary().scope().unwrap().clone();
    assert!(analyses.latest_for_document(&canonical).unwrap().is_some());

    let closed = documents.close(&source).unwrap();
    let invalidation = analyses.analyze(closed).unwrap();

    assert!(invalidation.primary().scope().is_none());
    assert!(invalidation.primary().snapshot().is_none());
    assert!(
        invalidation
            .current_generations()
            .all(|generation| generation.scope() != Some(&active_scope))
    );
    assert!(analyses.latest_for_document(&canonical).unwrap().is_none());
}

#[test]
fn analysis_rejects_out_of_order_and_foreign_revision_sequences() {
    let temporary = TemporaryDirectory::new();
    let first = temporary.path().join("first.nct");
    let second = temporary.path().join("second.nct");
    let text = "func main(): void { return }\n";
    fs::write(&first, text).unwrap();
    fs::write(&second, text).unwrap();

    let mut documents = DocumentWorkspace::new();
    let older = documents.open(&first, 1, text).unwrap();
    let newer = documents.open(&second, 1, text).unwrap();
    let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
    analyses.analyze(newer).unwrap();
    assert_eq!(
        analyses.analyze(older).unwrap_err(),
        WorkspaceRevisionError::NonIncreasing {
            current: GenerationId::new(2),
            received: GenerationId::new(1),
        }
    );
    assert_eq!(analyses.latest_generation, Some(GenerationId::new(2)));

    let mut foreign_documents = DocumentWorkspace::new();
    let foreign = foreign_documents.open(&first, 2, text).unwrap();
    assert_eq!(
        analyses.analyze(foreign).unwrap_err(),
        WorkspaceRevisionError::ForeignSequence
    );
    assert_eq!(analyses.latest_generation, Some(GenerationId::new(2)));
}

pub(super) fn configuration(root: &Path) -> WorkspaceConfiguration {
    configuration_with_standard(root, &standard_root())
}

fn configuration_with_standard(root: &Path, standard_root: &Path) -> WorkspaceConfiguration {
    WorkspaceConfiguration::resolve(
        [fs::canonicalize(root).unwrap()],
        WorkspaceToolchain::new(
            CompilationTarget::Arm64Darwin,
            root,
            StandardPackage::new(
                PackageIdentity::new("toolchain:std"),
                standard_root,
                nocter_test_support::repository_release_version(),
            ),
        ),
    )
    .unwrap()
}

fn standard_root() -> PathBuf {
    nocter_test_support::standard_library_root()
}

pub(super) struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    pub(super) fn new() -> Self {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nocter-language-server-analysis-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
