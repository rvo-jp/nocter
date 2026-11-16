//! The session consumes sealed v2 query evidence without entering released checking or targets.
use crate::*;
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_compiler_computation::{
    CompilerComputation, UnitAnalysisOutcome, UnitAnalysisProduct, ValueAccessAnalysis,
};
use nocter_discovery::DiscoveryRequest;
use nocter_filesystem::SourceOverlay;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_syntax::ParseGoal;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn query(body: &str) -> (Self, Arc<UnitAnalysisProduct>) {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("nocter-v2-session-{}-{serial}", std::process::id()));
        fs::create_dir_all(directory.join("app")).unwrap();
        fs::create_dir_all(directory.join("std")).unwrap();
        fs::write(
            directory.join("app/index.nct"),
            "#package: { name: \"app\", version: \"2.0.0\" }\nsee ./body.nct\nfunc run(): i32",
        )
        .unwrap();
        fs::write(
            directory.join("app/body.nct"),
            format!("see ./index.nct\n{body}"),
        )
        .unwrap();
        fs::write(
            directory.join("std/index.nct"),
            "#package: { name: \"std\", version: \"2.0.0\" }",
        )
        .unwrap();
        let fixture = Self(fs::canonicalize(directory).unwrap());
        let overlay = SourceOverlay::empty();
        let mut computation = CompilerComputation::new();
        let revision = computation.advance_sources(&overlay, 0).unwrap();
        let identity = |name: &str| PackageIdentity::new(format!("workspace:{name}"));
        let packages = {
            let mut provider = computation.source_syntax(&revision).unwrap();
            ResolvedPackageGraph::load_with_root_catalog(
                ["app", "std"]
                    .into_iter()
                    .map(|name| ResolvedPackageSpec::new(identity(name), fixture.0.join(name)))
                    .collect(),
                PackageRootCatalog::for_goal(overlay, ParseGoal::V2CoreUnit),
                revision.source_identity_domain(),
                &mut provider,
            )
            .unwrap()
        };
        let std = identity("std");
        let request = DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            packages,
            vec![ModuleIdentity::new(identity("app"), Vec::<&str>::new())],
            ToolchainInput::new(
                std.clone(),
                ModuleIdentity::new(std, Vec::<&str>::new()),
                Vec::new(),
                Vec::new(),
            ),
        );
        let discovered = computation.discover(&revision, request).unwrap();
        let product = computation.analyze(&discovered).unwrap();
        (fixture, product)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn checked_v2_source_is_available_without_fabricating_an_executable_target() {
    let (_fixture, product) = Fixture::query("func run(): i32 { 42 }");
    let analysis = analyze_unit_from_query(&product).unwrap();
    assert_eq!(analysis.status(), AnalyzedUnitStatus::Complete);
    assert!(analysis.diagnostics().is_empty());
    assert!(analysis.semantic_evidence().is_none());
    assert!(matches!(
        analysis.value_access_analysis(),
        Some(ValueAccessAnalysis::Accepted(_))
    ));
    assert_eq!(
        analysis.unit().syntax_trees()[0].root_id(),
        product.unit().syntax_trees()[0].root_id()
    );
    let Err(failure) = analysis.into_compilation_result() else {
        panic!("unqualified target")
    };
    let (failure, _, diagnostics) = failure.into_parts();
    assert!(matches!(
        failure.primary(),
        CompileSessionError::ValueAccessTargetUnqualified
    ));
    assert!(diagnostics.is_empty());
    assert!(failure.diagnostic_code().is_none());
}

#[test]
fn v2_rejection_retains_the_query_report_and_exact_failure_envelope() {
    let (_fixture, product) = Fixture::query("func run(): i32 { missing }");
    let UnitAnalysisOutcome::ValueAccess(query) = product.outcome() else {
        panic!("v2 query")
    };
    let ValueAccessAnalysis::Rejected(report) = &**query else {
        panic!("typed rejection")
    };
    let analysis = analyze_unit_from_query(&product).unwrap();
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert_eq!(analysis.diagnostics(), report.diagnostics());
    assert!(analysis.semantic_evidence().is_none());
    let Err(failure) = analysis.into_compilation_result() else {
        panic!("invalid source")
    };
    let (failure, sources, diagnostics) = failure.into_parts();
    let CompileSessionError::ValueAccessSource(retained) = failure.primary() else {
        panic!("original report")
    };
    assert!(Arc::ptr_eq(retained, report));
    assert_eq!(&*diagnostics, report.diagnostics());
    assert_eq!(failure.source_diagnostics(), report.diagnostics());
    assert!(sources.get(diagnostics[0].primary().source()).is_some());
}

#[test]
fn incomplete_v2_source_retains_syntax_diagnostics_without_checked_evidence() {
    let (_fixture, product) = Fixture::query("func run(): i32 {");
    let analysis = analyze_unit_from_query(&product).unwrap();
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(matches!(
        analysis.value_access_analysis(),
        Some(ValueAccessAnalysis::IncompleteSyntax)
    ));
    assert_eq!(
        analysis.diagnostics(),
        &*product.unit().syntax_diagnostics()
    );
    assert!(analysis.semantic_evidence().is_none());
    let Err(failure) = analysis.into_compilation_result() else {
        panic!("incomplete source")
    };
    let (failure, _, diagnostics) = failure.into_parts();
    assert!(matches!(
        failure.primary(),
        CompileSessionError::SyntaxErrorsPresent
    ));
    assert!(!diagnostics.is_empty());
}
