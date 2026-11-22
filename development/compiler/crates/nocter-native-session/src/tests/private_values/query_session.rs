//! Physical v2 sources retain one checked authority through query, session and native consumers.
use super::{TempPackage, execute_native_status};
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_compiler_computation::{CompilerComputation, ValueAccessAnalysis};
use nocter_discovery::DiscoveryRequest;
use nocter_filesystem::SourceOverlay;
use nocter_model::{CompilationTarget, ConstantValue, FrozenNode, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_session::{
    ExecutableSelectionError, ExecutableSelector, ValueAccessEntryError,
    select_value_access_executable,
};
use nocter_syntax::ParseGoal;
use std::fs;

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
        vec![
            ModuleIdentity::new(identity("app"), Vec::<&str>::new()),
            ModuleIdentity::new(identity("app"), ["math"]),
        ],
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
    let Some(nocter_declarations::value_access::DeclarationId::Code(answer)) = exported("ANSWER")
    else {
        panic!("authored root constant contract")
    };
    assert_eq!(
        accepted.unit().constant_value(answer).unwrap().node(),
        &FrozenNode::Scalar(ConstantValue::Integer(42))
    );
    assert!(matches!(
        select_value_access_executable(&session, &ExecutableSelector::Only),
        Err(ValueAccessEntryError::Selection(
            ExecutableSelectionError::MultipleExecutables
        ))
    ));
    for (name, path, status) in [("app", &[][..], 42), ("math", &["math"][..], 17)] {
        let executable =
            select_value_access_executable(&session, &ExecutableSelector::named(name)).unwrap();
        assert_eq!(executable.declaration().name(), name);
        assert_eq!(
            executable.declaration().module(),
            &ModuleIdentity::new(identity("app"), path.iter().copied())
        );
        let image = crate::compile_value_access_image(executable).unwrap();
        execute_native_status(&image, &directory.0, &format!("image-{name}"), status);
    }
}
