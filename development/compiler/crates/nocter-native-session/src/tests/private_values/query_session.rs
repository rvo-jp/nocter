//! Physical v2 sources retain one checked authority through query, session and native consumers.
use super::{TempPackage, execute_native_status, image, machine_entry};
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_compiler_computation::{CompilerComputation, ValueAccessAnalysis};
use nocter_discovery::DiscoveryRequest;
use nocter_filesystem::SourceOverlay;
use nocter_model::{CompilationTarget, ConstantValue, FrozenNode, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_syntax::ParseGoal;
use std::{fs, sync::Arc};

#[test]
fn physical_query_session_unit_executes_distinct_cross_module_capture_values() {
    let directory = TempPackage::new();
    let fixture = nocter_test_support::repository_root()
        .join("development/compiler/tests/fixtures/value-modules/query-session");
    for relative in [
        "app/index.nct",
        "app/body.nct",
        "app/math/index.nct",
        "app/math/body.nct",
        "std/index.nct",
    ] {
        directory.source(
            relative,
            &fs::read_to_string(fixture.join(relative)).unwrap(),
        );
    }
    let overlay = SourceOverlay::empty();
    let mut computation = CompilerComputation::new();
    let revision = computation.advance_sources(&overlay, 0).unwrap();
    let identity = |name: &str| PackageIdentity::new(format!("workspace:{name}"));
    let packages = {
        let mut provider = computation.source_syntax(&revision).unwrap();
        ResolvedPackageGraph::load_with_root_catalog(
            ["app", "std"]
                .into_iter()
                .map(|name| {
                    ResolvedPackageSpec::new(
                        identity(name),
                        fs::canonicalize(directory.0.join(name)).unwrap(),
                    )
                })
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
    let source = computation.discover(&revision, request).unwrap();
    let product = computation.analyze(&source).unwrap();
    let session = nocter_session::analyze_unit_from_query(&product).unwrap();
    assert!(session.diagnostics().is_empty());
    let Some(ValueAccessAnalysis::Accepted(accepted)) = session.value_access_analysis() else {
        panic!("checked v2 session")
    };
    let program = accepted.headers().program();
    let module = ModuleIdentity::new(identity("app"), Vec::<&str>::new());
    let exported = |name| {
        accepted.headers().lookup_export(
            &module,
            &module,
            nocter_declarations::value_access::Namespace::Value,
            program.symbols().get(name).unwrap(),
        )
    };
    let Some(nocter_declarations::value_access::DeclarationId::Code(root)) = exported("run") else {
        panic!("authored root function contract")
    };
    let Some(nocter_declarations::value_access::DeclarationId::Code(answer)) = exported("ANSWER")
    else {
        panic!("authored root constant contract")
    };
    assert_eq!(
        accepted.unit().constant_value(answer).unwrap().node(),
        &FrozenNode::Scalar(ConstantValue::Integer(42))
    );
    // Consume the already checked Unit. No SourceUnit/ModuleSourceUnit preparation occurs here.
    let machine = machine_entry(Arc::new(accepted.unit().clone()), root).unwrap();
    execute_native_status(&image(&machine), &directory.0, "query-session-captures", 42);
}
