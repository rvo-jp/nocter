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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const INVALID_PROCESS_TEXT: &[u8] =
    b"std.process.invalid_encoding: process text or requested environment name is invalid\n";

fn copy_fixture(directory: &TempPackage) {
    let fixture = nocter_test_support::repository_root()
        .join("development/compiler/tests/fixtures/value-modules/query-session");
    let mut folders = vec![fixture.clone()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            let path = entry.path();
            if kind.is_dir() {
                folders.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "nct")
            {
                let relative = path.strip_prefix(&fixture).unwrap().to_str().unwrap();
                directory.source(relative, &fs::read_to_string(&path).unwrap());
            }
        }
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
            PrimitiveRole::ProcessArgument,
            ModuleIdentity::new(std.clone(), ["process"]),
            "arg_checked",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessEnvironmentCount,
            ModuleIdentity::new(std.clone(), ["process"]),
            "environment_count",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessEnvironmentName,
            ModuleIdentity::new(std.clone(), ["process"]),
            "env_name_checked",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::ProcessEnvironmentValue,
            ModuleIdentity::new(std.clone(), ["process"]),
            "env_value_checked",
        ),
        PrimitiveRoleLocator::new(
            PrimitiveRole::StringByteAt,
            ModuleIdentity::new(std, ["process"]),
            "text_byte_at",
        ),
    ])
}

fn analyze_package(
    directory: &TempPackage,
    target: CompilationTarget,
) -> (nocter_session::AnalyzedUnit, PackageIdentity) {
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
    let declaration = packages
        .packages()
        .iter()
        .find(|package| package.identity() == &app)
        .and_then(|package| package.declaration())
        .unwrap();
    let roots = declaration
        .targets()
        .iter()
        .map(|target| ModuleIdentity::new(app.clone(), target.module().iter().cloned()))
        .collect();
    let request = DiscoveryRequest::declared(target, packages, roots, standard_profile(std));
    let source = computation.discover(&revision, request).unwrap();
    let product = computation.analyze(&source).unwrap();
    let session = nocter_session::analyze_unit_from_query(&product).unwrap();
    (session, app)
}

#[test]
fn physical_query_session_unit_executes_selected_entries() {
    let directory = TempPackage::new();
    copy_fixture(&directory);
    let (session, app) = analyze_package(&directory, CompilationTarget::Arm64Darwin);
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
        (
            "managed-deferred-failure",
            &b"managed: after await: leaf\n"[..],
        ),
        ("compound-deferred", &b"compound: payload\n"[..]),
    ] {
        let image =
            crate::compile_value_access_image(&session, &ExecutableSelector::named(name)).unwrap();
        assert_failure_report(image.image(), &directory.0, name, error);
    }
    assert_selected_future_cleanup(
        &session,
        "managed-deferred-failure",
        |layout| matches!(layout, nocter_machine::MachineLayoutKind::ErrorHandle),
        4,
        2,
    );
    assert_selected_future_cleanup(
        &session,
        "compound-deferred",
        |layout| matches!(layout, nocter_machine::MachineLayoutKind::Struct { fields } if fields.len() == 3),
        3,
        1,
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn v2_argument_text_preserves_boundaries_and_rejects_invalid_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = TempPackage::new();
    copy_fixture(&directory);
    let (session, _) = analyze_package(&directory, CompilationTarget::Arm64Darwin);
    assert!(
        session.diagnostics().is_empty(),
        "{:#?}",
        session.diagnostics()
    );
    let image =
        crate::compile_value_access_image(&session, &ExecutableSelector::named("arguments"))
            .unwrap();
    let launch = |name: &str, value: Option<&[u8]>| {
        let mut command = super::super::native_command(image.image(), &directory.0, name);
        if let Some(value) = value {
            command.arg(OsStr::from_bytes(value));
            // A malformed neighbor cannot affect the selected argument.
            command.arg(OsStr::from_bytes(b"\xff"));
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty());
        output.stderr
    };
    assert_eq!(launch("missing", None), b"argv: absent\n");
    for text in ["", "two words", "Å✓🦉", "\u{80}\u{800}\u{10000}\u{10ffff}"] {
        assert_eq!(
            launch("valid-utf8", Some(text.as_bytes())),
            format!("argv: {text}\n").as_bytes(),
            "{text:?}"
        );
    }
    for bytes in [
        &b"\xff"[..],
        &b"\x80"[..],
        &b"\xc0\xaf"[..],
        &b"\xe0\x80\x80"[..],
        &b"\xed\xa0\x80"[..],
        &b"\xf0\x80\x80\x80"[..],
        &b"\xf4\x90\x80\x80"[..],
        &b"\xe2\x82"[..],
        &b"valid prefix \xe2 "[..],
    ] {
        assert_eq!(
            launch("invalid-utf8", Some(bytes)),
            INVALID_PROCESS_TEXT,
            "{bytes:?}"
        );
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn v2_environment_text_keeps_the_first_separator_and_checks_each_part() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = TempPackage::new();
    copy_fixture(&directory);
    let (session, _) = analyze_package(&directory, CompilationTarget::Arm64Darwin);
    assert!(
        session.diagnostics().is_empty(),
        "{:#?}",
        session.diagnostics()
    );
    let image =
        crate::compile_value_access_image(&session, &ExecutableSelector::named("environment"))
            .unwrap();
    let launch = |name: &str, binding: Option<(&[u8], &[u8])>| {
        let mut command = super::super::native_command(image.image(), &directory.0, name);
        command.env_clear();
        if let Some((key, value)) = binding {
            command.env(OsStr::from_bytes(key), OsStr::from_bytes(value));
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty());
        output.stderr
    };
    assert_eq!(launch("no-entry", None), b"env: absent\n");
    for (key, value) in [("NOCTER", ""), ("NOCTER", "prefix=value"), ("Å", "✓🦉")] {
        assert_eq!(
            launch("valid-entry", Some((key.as_bytes(), value.as_bytes()))),
            format!("{key}: {value}\n").as_bytes(),
        );
    }
    for (key, value) in [
        (&b"\xff"[..], &b"valid"[..]),
        (&b"prefix\xe2\x82"[..], &b"valid"[..]),
        (&b"NOCTER"[..], &b"prefix\xed\xa0\x80"[..]),
    ] {
        assert_eq!(
            launch("invalid-entry", Some((key, value))),
            INVALID_PROCESS_TEXT
        );
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn v2_environment_lookup_keeps_exact_names_and_original_validation_order() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = TempPackage::new();
    copy_fixture(&directory);
    let (session, _) = analyze_package(&directory, CompilationTarget::Arm64Darwin);
    assert!(
        session.diagnostics().is_empty(),
        "{:#?}",
        session.diagnostics()
    );
    let image = crate::compile_value_access_image(
        &session,
        &ExecutableSelector::named("environment-lookup"),
    )
    .unwrap();
    let launch = |arguments: &[&str], entries: &[(&str, &[u8])]| {
        let output = super::super::native_command(image.image(), &directory.0, "env-lookup")
            .env_clear()
            .args(arguments)
            .envs(
                entries
                    .iter()
                    .map(|(key, bytes)| (*key, OsStr::from_bytes(bytes))),
            )
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{arguments:?}, {entries:?}");
        assert!(output.stdout.is_empty());
        output.stderr
    };
    assert_eq!(launch(&[], &[]), b"env: absent\n");
    assert_eq!(launch(&[], &[("NOCTER", b"")]), b"env: \n");
    assert_eq!(
        launch(&["Å"], &[("Å", "夜".as_bytes())]),
        "env: 夜\n".as_bytes()
    );
    assert_eq!(
        launch(&["NOCT"], &[("NOCTER", b"prefix")]),
        b"env: absent\n"
    );
    assert_eq!(
        launch(&["NOCTERS"], &[("NOCTER", b"prefix")]),
        b"env: absent\n"
    );
    for invalid_name in [&[""][..], &["A=B"][..], &["use", "embedded-zero"][..]] {
        assert_eq!(launch(invalid_name, &[]), INVALID_PROCESS_TEXT);
    }
    assert_eq!(
        launch(&[], &[("AAA", b"\xff"), ("NOCTER", b"available")]),
        INVALID_PROCESS_TEXT
    );
    assert_eq!(
        launch(&[], &[("NOCTER", b"available"), ("ZZZ", b"\xff")]),
        b"env: available\n"
    );
}

fn assert_selected_future_cleanup(
    session: &nocter_session::AnalyzedUnit,
    executable: &str,
    is_capture: impl Fn(&nocter_machine::MachineLayoutKind) -> bool,
    total_calls: usize,
    selected_calls: usize,
) {
    use nocter_machine::{
        MachineBlockId, MachineCallTarget, MachineOperationKind, MachineProgramRoot,
        MachineTerminator, MachineValueCarrier,
    };
    let executable =
        select_value_access_executable(session, &ExecutableSelector::named(executable)).unwrap();
    let (unit, code) = executable.into_entry();
    let machine = super::machine_entry(unit, code).unwrap();
    let MachineProgramRoot::Process { entry, .. } = *machine.root() else {
        panic!("expected a process entry");
    };
    let mut helpers = machine.functions().filter(|(_, function)| {
        let body = function.body();
        let [parameter] = body.parameters() else {
            return false;
        };
        body.blocks()
            .any(|(_, block)| matches!(block.terminator(), MachineTerminator::Return(None)))
            && body.stack(*parameter).is_some_and(|slot| {
                slot.carrier() == MachineValueCarrier::Address
                    && machine
                        .layouts()
                        .get(slot.ty())
                        .is_some_and(|layout| is_capture(layout.kind()))
            })
    });
    let (release, _) = helpers
        .next()
        .expect("a helper releases the represented captured input");
    assert!(helpers.next().is_none());
    let body = machine.function(entry).unwrap().body();
    let count_releases = |block: MachineBlockId| {
        body.block(block)
            .unwrap()
            .operations()
            .iter()
            .filter(|&&id| {
                matches!(body.operation(id).unwrap().kind(), MachineOperationKind::Call(call)
                if *call.target() == MachineCallTarget::Direct(release))
            })
            .count()
    };
    assert_eq!(
        body.blocks()
            .map(|(id, _)| count_releases(id))
            .sum::<usize>(),
        total_calls
    );
    assert!(
        body.blocks().any(|(_, block)| {
            let MachineTerminator::SwitchValue { cases, .. } = block.terminator() else {
                return false;
            };
            let mut counts = cases
                .iter()
                .map(|case| count_releases(case.target().block()))
                .collect::<Vec<_>>();
            counts.sort_unstable();
            counts == [0, selected_calls]
        }),
        "only the selected resource-owning case releases captures"
    );
}

#[test]
fn a_different_selected_target_cannot_use_arm64_darwin_primitive_drive_evidence() {
    let directory = TempPackage::new();
    copy_fixture(&directory);
    for module in [
        "managed_deferred_failure",
        "compound_deferred",
        "arguments",
        "environment",
        "environment_lookup",
    ] {
        directory.source(
            &format!("app/{module}/body.nct"),
            "see ./index.nct\nasync func main(): i32! { 0 }",
        );
    }
    let (session, _) = analyze_package(&directory, CompilationTarget::X64Linux);
    let [diagnostic] = session.diagnostics() else {
        panic!("{:#?}", session.diagnostics());
    };
    assert_eq!(diagnostic.code(), "E0291");
    let span = diagnostic.primary().span();
    assert!(matches!(
        session
            .unit()
            .sources()
            .get(span.source())
            .unwrap()
            .text_at(span.range()),
        Some(
            "process.arg_count()"
                | "process.environment_count()"
                | "errors.operation()"
                | "static_error.operation()"
        )
    ));
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
