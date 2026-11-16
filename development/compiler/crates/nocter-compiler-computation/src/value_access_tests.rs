//! Revision-owned v2 analysis uses the ordinary source/discovery/query entry.
use super::*;
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_filesystem::SourceOverride;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_syntax::ParseGoal;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
const BODY: &str = "see ./index.nct\nconst func value(take amount: i32): i32 { 40 + amount }\nconst RESULT: i32 = value(2)";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "nocter-v2-computation-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("std")).unwrap();
        fs::write(root.join("app/index.nct"), "#package: { name: \"app\", version: \"2.0.0\" }\nsee ./body.nct\nconst func value(take amount: i32): i32\nconst RESULT: i32").unwrap();
        fs::write(
            root.join("std/index.nct"),
            "#package: { name: \"std\", version: \"2.0.0\" }",
        )
        .unwrap();
        Self(fs::canonicalize(root).unwrap())
    }

    fn load(&self, computation: &mut CompilerComputation, body: &str) -> CompilerDiscoveredUnit {
        let mut overlay = SourceOverlay::builder();
        overlay
            .insert_source(
                self.0.join("app/body.nct"),
                SourceOverride::new(body.as_bytes()),
            )
            .unwrap();
        let overlay = overlay.finish();
        let revision = computation.advance_sources(&overlay, 0).unwrap();
        let identity = |name: &str| PackageIdentity::new(format!("workspace:{name}"));
        let packages = {
            let mut provider = computation.source_syntax(&revision).unwrap();
            ResolvedPackageGraph::load_with_root_catalog(
                ["app", "std"]
                    .into_iter()
                    .map(|name| ResolvedPackageSpec::new(identity(name), self.0.join(name)))
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
        computation.discover(&revision, request).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn analysis(product: &UnitAnalysisProduct) -> &Arc<ValueAccessAnalysis> {
    let UnitAnalysisOutcome::ValueAccess(analysis) = product.outcome() else {
        panic!("selected v2 analysis")
    };
    analysis
}
fn accepted(product: &UnitAnalysisProduct) -> &AcceptedValueAccess {
    let ValueAccessAnalysis::Accepted(accepted) = &**analysis(product) else {
        panic!("complete v2 checking")
    };
    accepted
}
fn result(product: &UnitAnalysisProduct) -> i128 {
    let accepted = accepted(product);
    let context = accepted
        .headers()
        .bind(product.unit().compile_input().unwrap())
        .unwrap();
    let current = context.current().unwrap();
    accepted.unit().bind(&current).unwrap();
    let program = current.program();
    let id = program
        .resolve_constant_in(
            current.lookup_scope(),
            program.symbols().get("RESULT").unwrap(),
        )
        .unwrap();
    let nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(value)) =
        accepted.unit().constant_value(id).unwrap().node()
    else {
        panic!("integer constant")
    };
    *value
}

#[test]
fn v2_query_preserves_required_values_reuse_and_current_source_guards() {
    let fixture = Fixture::new();
    let mut computation = CompilerComputation::new();
    let before = fixture.load(&mut computation, BODY);
    let first = computation.analyze(&before).unwrap();
    assert_eq!(result(&first), 42);
    let again = computation.analyze(&before).unwrap();
    assert!(Arc::ptr_eq(analysis(&first), analysis(&again)));
    assert_eq!(computation.statistics().declaration_executions, 0);
    let after = fixture.load(
        &mut computation,
        &BODY.replace("40 + amount", "39 + amount"),
    );
    let second = computation.analyze(&after).unwrap();
    assert_eq!(result(&second), 41);
    assert!(!Arc::ptr_eq(analysis(&first), analysis(&second)));
    // Header reuse is valid for a body-only edit; old complete body/value evidence is not.
    let rebound = accepted(&first)
        .headers()
        .bind(after.compile_input().unwrap())
        .unwrap();
    assert!(
        accepted(&first)
            .unit()
            .bind(&rebound.current().unwrap())
            .is_err()
    );
    assert!(matches!(
        computation.analyze(&before),
        Err(CompilerComputationError::SourceRevision(_))
    ));
}

#[test]
fn v2_query_retains_incomplete_syntax_without_running_released_recovery() {
    let fixture = Fixture::new();
    let mut computation = CompilerComputation::new();
    let source = fixture.load(&mut computation, "see ./index.nct\nconst RESULT: i32 =");
    assert!(source.has_syntax_errors());
    let product = computation.analyze(&source).unwrap();
    assert!(matches!(
        &**analysis(&product),
        ValueAccessAnalysis::IncompleteSyntax
    ));
    assert!(!product.unit().syntax_diagnostics().is_empty());
    let statistics = computation.statistics();
    assert_eq!(statistics.declaration_executions, 0);
    assert_eq!(statistics.incomplete_analysis_executions, 0);
}

#[test]
fn v2_query_preserves_unused_body_rejection_at_the_actual_source() {
    use nocter_checking::value_access::{Failure, Rule, accepted::SourceFailure};
    let fixture = Fixture::new();
    let mut computation = CompilerComputation::new();
    let source = fixture.load(
        &mut computation,
        &format!("{BODY}\nfunc unused(): i32 {{ missing }}"),
    );
    let product = computation.analyze(&source).unwrap();
    let ValueAccessAnalysis::Rejected(SourceFailure::Checking(
        nocter_checking::value_access::accepted::Failure::Body {
            failure: Failure::Rejected(error),
            ..
        },
    )) = &**analysis(&product)
    else {
        panic!("authored body rejection")
    };
    assert_eq!(error.rule(), Rule::UnknownName);
    assert_eq!(
        product
            .unit()
            .sources()
            .get(error.source().primary().source())
            .unwrap()
            .name()
            .as_str(),
        fixture.0.join("app/body.nct").to_str().unwrap()
    );
    assert_eq!(computation.statistics().declaration_executions, 0);
    let repaired = fixture.load(&mut computation, BODY);
    let healthy = computation.analyze(&repaired).unwrap();
    assert_eq!(result(&healthy), 42);
    assert!(!Arc::ptr_eq(analysis(&product), analysis(&healthy)));
}
