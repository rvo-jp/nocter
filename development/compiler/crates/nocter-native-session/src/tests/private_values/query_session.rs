//! Physical v2 sources retain one checked authority through query, session and native consumers.
use super::{TempPackage, execute_native_status};
use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
use nocter_compiler_computation::{CompilerComputation, ValueAccessAnalysis};
use nocter_discovery::DiscoveryRequest;
use nocter_filesystem::SourceOverlay;
use nocter_model::{CompilationTarget, ConstantValue, FrozenNode, PackageIdentity};
use nocter_package::{PackageRootCatalog, ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::PrimitiveRole;
use nocter_session::{
    ExecutableSelectionError, ExecutableSelector, ValueAccessEntryError,
    select_value_access_executable,
};
use nocter_syntax::ParseGoal;
use std::fs;

fn copy_fixture(directory: &TempPackage) {
    let fixture = nocter_test_support::repository_root()
        .join("development/compiler/tests/fixtures/value-modules/query-session");
    for relative in [
        "app/index.nct",
        "app/body.nct",
        "app/math/index.nct",
        "app/math/body.nct",
        "app/failure/index.nct",
        "app/failure/body.nct",
        "app/deferred_failure/index.nct",
        "app/deferred_failure/body.nct",
        "app/later/index.nct",
        "app/later/body.nct",
        "std/index.nct",
        "std/process/index.nct",
    ] {
        directory.source(
            relative,
            &fs::read_to_string(fixture.join(relative)).unwrap(),
        );
    }
    let trusted_errors = fixture.with_file_name("trusted-errors");
    for module in ["error", "context", "allocation_failure"] {
        let index = format!("{module}/index.nct");
        directory.source(
            &format!("std/{index}"),
            &fs::read_to_string(trusted_errors.join(index)).unwrap(),
        );
    }
}

fn standard_profile(std: PackageIdentity) -> ToolchainInput {
    ToolchainInput::new(
        std.clone(),
        ModuleIdentity::new(std.clone(), Vec::<&str>::new()),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![
        PrimitiveRoleLocator::new(
            PrimitiveRole::NewError,
            ModuleIdentity::new(std.clone(), ["error"]),
            "operation",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ErrorContext,
            ModuleIdentity::new(std.clone(), ["context"]),
            "operation",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::AllocationFailureError,
            ModuleIdentity::new(std.clone(), ["allocation_failure"]),
            "operation",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessArgumentCount,
            ModuleIdentity::new(std.clone(), ["process"]),
            "arg_count",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessEnvironmentCount,
            ModuleIdentity::new(std, ["process"]),
            "environment_count",
        ),
    ])
}

fn analyze_package(directory: &TempPackage) -> (nocter_session::AnalyzedUnit, PackageIdentity) {
    let overlay = SourceOverlay::empty();
    let mut computation = CompilerComputation::new();
    let revision = computation.advance_sources(&overlay, 0).unwrap();
    let identity = |name: &str| PackageIdentity::new(format!("workspace:{name}"));
    let app = identity("app");
    let std = identity("std");
    let package = |name| {
        ResolvedPackageSpec::new(
            identity(name),
            fs::canonicalize(directory.0.join(name)).unwrap(),
        )
    };
    let packages = {
        let mut provider = computation.source_syntax(&revision).unwrap();
        ResolvedPackageGraph::load_with_root_catalog(
            vec![
                package("app").with_standard_dependency(std.clone()),
                package("std"),
            ],
            PackageRootCatalog::for_goal(overlay, ParseGoal::V2CoreUnit),
            revision.source_identity_domain(),
            &mut provider,
        )
        .unwrap()
    };
    let request = DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        packages,
        vec![
            ModuleIdentity::new(app.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(app.clone(), ["math"]),
            ModuleIdentity::new(app.clone(), ["failure"]),
            ModuleIdentity::new(app.clone(), ["deferred_failure"]),
            ModuleIdentity::new(app.clone(), ["later"]),
        ],
        standard_profile(std),
    );
    let source = computation.discover(&revision, request).unwrap();
    let product = computation.analyze(&source).unwrap();
    let session = nocter_session::analyze_unit_from_query(&product).unwrap();
    (session, app)
}

#[test]
fn physical_query_session_unit_executes_selected_entries() {
    let directory = TempPackage::new();
    copy_fixture(&directory);
    let (session, app) = analyze_package(&directory);
    assert!(
        session.diagnostics().is_empty(),
        "{:#?}",
        session.diagnostics()
    );
    let Some(ValueAccessAnalysis::Accepted(accepted)) = session.value_access_analysis() else {
        panic!("checked v2 session")
    };
    let program = accepted.headers().program();
    let module = ModuleIdentity::new(app.clone(), Vec::<&str>::new());
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
    for (name, path, status) in [
        ("app", &[][..], 42),
        ("math", &["math"][..], 17),
        ("later", &["later"][..], 28),
    ] {
        let selector = ExecutableSelector::named(name);
        let executable = select_value_access_executable(&session, &selector).unwrap();
        let declaration = executable.package_declaration().unwrap();
        assert_eq!(declaration.name(), name);
        assert_eq!(
            declaration.module(),
            &ModuleIdentity::new(app.clone(), path.iter().copied())
        );
        let image = crate::compile_value_access_image(&session, &selector)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(image.identity().name(), name);
        assert_eq!(image.identity().package(), &app);
        if name == "math" {
            assert_process_context(image.image(), &directory.0, status);
        } else {
            execute_native_status(
                image.image(),
                &directory.0,
                &format!("image-{name}"),
                status,
            );
        }
    }
    for (name, error) in [
        ("failure", &b"failed: outer: leaf\n"[..]),
        (
            "deferred-failure",
            &b"std.mem.out_of_memory: allocation failed\n"[..],
        ),
    ] {
        let image =
            crate::compile_value_access_image(&session, &ExecutableSelector::named(name)).unwrap();
        assert_failure_report(image.image(), &directory.0, name, error);
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_process_context(image: &crate::NativeImage, root: &std::path::Path, base_status: i32) {
    for (name, arguments, environment, expected) in [
        ("plain", &[][..], &[][..], base_status),
        (
            "with-args",
            &["--example", "two words", ""][..],
            &[][..],
            base_status + 3,
        ),
        (
            "with-environment",
            &[][..],
            &[("NOCTER_FIRST", ""), ("NOCTER_SECOND", "two words")][..],
            base_status + 16,
        ),
    ] {
        let status = super::super::native_command(image, root, &format!("image-{name}"))
            .env_clear()
            .args(arguments)
            .envs(environment.iter().copied())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(expected), "{name}");
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn assert_process_context(_image: &crate::NativeImage, _root: &std::path::Path, _base_status: i32) {
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_failure_report(
    image: &crate::NativeImage,
    root: &std::path::Path,
    name: &str,
    error: &[u8],
) {
    let output = super::super::native_command(image, root, &format!("image-{name}"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, error);
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn assert_failure_report(
    _image: &crate::NativeImage,
    _root: &std::path::Path,
    _name: &str,
    _error: &[u8],
) {
}
