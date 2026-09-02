use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::DiscoveryRequest;
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_native_session::NativeImageSetCompileRequest;
use nocter_package::{
    ExactDependencyLock, PackageResolutionError, ResolvedPackageGraph, ResolvedPackageSpec,
    StandardPackage,
};
use nocter_package_state::{
    LockResolutionRequest, PackageAcquisitionAuthority, PackageFetchRequest,
};
use nocter_session::{ExecutableCompileRequest, ExecutableSelector};
use nocter_standard_profile::bundled_standard_toolchain;
use nocter_test_support::{
    PUBLIC_PACKAGE_EXAMPLES, PublicExampleArgument, PublicExampleFixture, PublicPackageExample,
    repository_release_version,
};

use super::artifact::persist_bytes;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

struct NoRemoteAcquisition;

const ARCHIVE_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[derive(Default)]
struct FixtureAcquisition {
    lock_calls: usize,
    fetch_calls: usize,
}

impl PackageAcquisitionAuthority for NoRemoteAcquisition {
    type Error = std::io::Error;

    fn resolve_lock(
        &mut self,
        _request: LockResolutionRequest<'_>,
    ) -> Result<ExactDependencyLock, Self::Error> {
        panic!("test does not authorize remote lock resolution")
    }

    fn fetch_package(&mut self, _request: PackageFetchRequest<'_>) -> Result<(), Self::Error> {
        panic!("test does not authorize remote package fetching")
    }
}

impl PackageAcquisitionAuthority for FixtureAcquisition {
    type Error = std::io::Error;

    fn resolve_lock(
        &mut self,
        request: LockResolutionRequest<'_>,
    ) -> Result<ExactDependencyLock, Self::Error> {
        assert!(request.workspace().is_dir());
        self.lock_calls += 1;
        ExactDependencyLock::sha256(ARCHIVE_DIGEST)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn fetch_package(&mut self, request: PackageFetchRequest<'_>) -> Result<(), Self::Error> {
        assert_eq!(request.lock().value(), ARCHIVE_DIGEST);
        assert!(request.workspace().is_dir());
        self.fetch_calls += 1;
        fs::write(
            request.destination().join("index.nct"),
            "//! Remote package.\n#package: { name: \"remote\", version: \"0.0.0\", }\n",
        )
    }
}

#[test]
fn persistent_bytes_replace_an_existing_artifact_atomically() {
    let directory = unique_test_directory("replace");
    let output = directory.join("program");
    fs::write(&output, b"previous").unwrap();

    persist_bytes(b"complete image", &output).unwrap();

    assert_eq!(fs::read(&output).unwrap(), b"complete image");
    assert_eq!(temporary_entries(&directory), Vec::<String>::new());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_output_selection_does_not_create_a_temporary_artifact() {
    let directory = unique_test_directory("invalid");

    let error = persist_bytes(b"image", std::path::Path::new("")).unwrap_err();

    assert_eq!(error.operation(), super::ArtifactOperation::SelectOutput);
    assert_eq!(temporary_entries(&directory), Vec::<String>::new());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn default_program_input_selects_only_the_exact_current_package() {
    let directory = unique_test_directory("input-package");
    fs::write(
        directory.join("index.nct"),
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    )
    .unwrap();

    let selected = super::resolve_program_input(
        &directory,
        super::ProgramInputOptions::package(None::<PathBuf>),
    )
    .unwrap();

    let package = selected.package().unwrap();
    assert_eq!(package.root(), fs::canonicalize(&directory).unwrap());
    assert_eq!(package.declaration(), package.root().join("index.nct"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_input_never_searches_an_ancestor_for_nocter_nct() {
    let directory = unique_test_directory("input-no-ancestor");
    fs::write(
        directory.join("index.nct"),
        "#package: { name: \"parent\", version: \"0.0.0\", }\n",
    )
    .unwrap();
    let child = directory.join("child");
    fs::create_dir(&child).unwrap();

    let error =
        super::resolve_program_input(&child, super::ProgramInputOptions::package(None::<PathBuf>))
            .unwrap_err();

    assert!(matches!(
        error,
        super::ProgramInputError::MissingPackageDeclaration(path)
            if path == fs::canonicalize(&child).unwrap().join("index.nct")
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn positional_and_explicit_file_forms_converge_without_permitting_conflicts() {
    let directory = unique_test_directory("input-file");
    fs::write(directory.join("app.nct"), "func main(): void { return }\n").unwrap();
    let positional = super::resolve_program_input(
        &directory,
        super::ProgramInputOptions::positional_file("app.nct"),
    )
    .unwrap();
    let explicit = super::resolve_program_input(
        &directory,
        super::ProgramInputOptions::explicit_file("app.nct"),
    )
    .unwrap();
    let standalone = super::resolve_single_file_input(&directory, "app.nct").unwrap();

    assert_eq!(positional, explicit);
    assert_eq!(positional.single_file(), Some(&standalone));
    assert!(matches!(
        super::resolve_program_input(
            &directory,
            super::ProgramInputOptions::new(None, Some("app.nct".into()), Some("app.nct".into()))
        )
        .unwrap_err(),
        super::ProgramInputError::ConflictingFileForms
    ));
    assert!(matches!(
        super::resolve_program_input(
            &directory,
            super::ProgramInputOptions::new(Some(".".into()), Some("app.nct".into()), None)
        )
        .unwrap_err(),
        super::ProgramInputError::RootWithFile
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn build_and_run_plans_close_package_and_file_selection_rules() {
    let directory = unique_test_directory("command-plans");
    let package_root = directory.join("package");
    fs::create_dir(&package_root).unwrap();
    fs::write(
        package_root.join("index.nct"),
        "#package: { name: \"app\", version: \"0.0.0\", }\n",
    )
    .unwrap();
    fs::write(
        directory.join("script.nct"),
        "func main(): void { return }\n",
    )
    .unwrap();
    let canonical_directory = fs::canonicalize(&directory).unwrap();
    let canonical_package = fs::canonicalize(&package_root).unwrap();

    let package = || {
        super::resolve_program_input(
            &directory,
            super::ProgramInputOptions::package(Some("package")),
        )
        .unwrap()
    };
    let all =
        super::BuildCommandPlan::new(package(), super::BuildCommandOptions::default()).unwrap();
    assert!(matches!(
        all.operation(),
        super::BuildOperation::PackageSet { output_directory }
            if output_directory == &canonical_package
    ));

    let selected =
        super::BuildCommandPlan::new(package(), super::BuildCommandOptions::executable("tool"))
            .unwrap();
    assert!(matches!(
        selected.operation(),
        super::BuildOperation::Selected {
            selector: ExecutableSelector::Named(name),
            output: super::SelectedBuildOutput::TargetNameIn(directory),
        } if name.as_ref() == "tool" && directory == &canonical_package
    ));

    let explicit =
        super::BuildCommandPlan::new(package(), super::BuildCommandOptions::output("bin/tool"))
            .unwrap();
    assert!(matches!(
        explicit.operation(),
        super::BuildOperation::Selected {
            selector: ExecutableSelector::Only,
            output: super::SelectedBuildOutput::Exact(path),
        } if path == &canonical_directory.join("bin/tool")
    ));

    let run = super::RunCommandPlan::new(
        package(),
        super::RunCommandOptions::executable("tool"),
        super::RunProgramArguments::new([std::ffi::OsString::from("input")]),
    )
    .unwrap();
    assert!(matches!(run.selector(), ExecutableSelector::Named(name) if name.as_ref() == "tool"));
    assert_eq!(run.working_directory(), canonical_package);
    assert_eq!(
        run.program_arguments().as_slice(),
        [std::ffi::OsString::from("input")]
    );

    let file = || {
        super::resolve_program_input(
            &directory,
            super::ProgramInputOptions::positional_file("script.nct"),
        )
        .unwrap()
    };
    let script =
        super::BuildCommandPlan::new(file(), super::BuildCommandOptions::default()).unwrap();
    assert!(matches!(
        script.operation(),
        super::BuildOperation::Selected {
            selector: ExecutableSelector::Only,
            output: super::SelectedBuildOutput::Exact(path),
        } if path == &canonical_directory.join("script")
    ));
    assert!(matches!(
        super::RunCommandPlan::new(
            file(),
            super::RunCommandOptions::executable("forbidden"),
            super::RunProgramArguments::default(),
        )
        .unwrap_err(),
        super::CommandPlanError::ExecutableWithSingleFile
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn build_commits_one_complete_session_image_at_the_requested_path() {
    let mut compiler = super::compiler::CommandCompiler::default();
    let unit = compiler.discover(hello_request()).unwrap();
    let directory = unique_test_directory("build");
    let output = directory.join("hello");

    let target = compiler.compile(&unit).unwrap();
    let built = super::build_executable(ExecutableCompileRequest::only(target), &output).unwrap();

    assert_eq!(built.artifact().path(), output);
    assert_eq!(&fs::read(&output).unwrap()[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
    assert_eq!(temporary_entries(&directory), Vec::<String>::new());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn one_shot_command_demands_the_closed_semantic_query_graph() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = compiler_root.join("../../examples/hello.nct");
    let standard_root = compiler_root.join("../std");
    let standard = PackageIdentity::new("toolchain:std");
    let mut compiler = super::compiler::CommandCompiler::default();
    let unit = compiler
        .discover(DiscoveryRequest::single_file(
            CompilationTarget::Arm64Darwin,
            source,
            package_graph(vec![resolved_standard(&standard_root, &standard)]),
            bundled_standard_toolchain(&standard),
        ))
        .unwrap();

    compiler.compile(&unit).unwrap();

    let statistics = compiler.statistics();
    assert!(statistics.source_text_executions > 0);
    assert!(statistics.parse_executions > 0);
    assert!(statistics.declaration_surface_executions > 0);
    assert!(statistics.module_surface_executions > 0);
    assert_eq!(statistics.declaration_executions, 1);
    assert_eq!(statistics.preparation_executions, 1);
    assert!(statistics.body_name_executions > 0);
    assert!(statistics.typed_body_executions > 0);
    assert_eq!(statistics.finalization_executions, 1);
    assert_eq!(statistics.complete_analysis_executions, 1);
    assert_eq!(statistics.incomplete_analysis_executions, 0);
    assert_eq!(statistics.unit_analysis_executions, 1);
}

#[test]
fn command_failure_retains_diagnostics_selected_beneath_incomplete_syntax() {
    let directory = unique_test_directory("closed-diagnostic-envelope");
    let source = directory.join("broken.nct");
    write_source(
        &directory,
        "broken.nct",
        concat!(
            "func inspect(value: &str): void {\n",
            "    absent()\n",
            "    value.\n",
            "    return\n",
            "}\n",
        ),
    );
    let (mut compiler, unit) = command_discover(single_file_request(&source));
    let syntax_diagnostic_count = unit.syntax_diagnostics().len();

    let failure = compiler.compile(&unit).unwrap_err();

    assert!(syntax_diagnostic_count > 0);
    assert!(
        failure.diagnostics().len() > syntax_diagnostic_count,
        "the closed session failure must retain semantic recovery diagnostics"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_build_publishes_all_executables_in_declaration_order() {
    let directory = unique_test_directory("build-set");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"tools\", version: \"0.0.0\", }\n#executable: { name: \"first\", module: \"./src/first\" }\n#executable: { name: \"second\", module: \"./src/second\" }\n",
    );
    write_source(&package_root, "index.nct", "//! Tools package.\n");
    write_source(
        &package_root,
        "src/first/index.nct",
        "func main(): void { return }\n",
    );
    write_source(
        &package_root,
        "src/second/index.nct",
        "func main(): void { return }\n",
    );
    let (mut compiler, unit) = command_discover(package_request(
        vec![package("workspace:tools", "tools", &package_root)],
        vec![
            module("workspace:tools", &[]),
            module("workspace:tools", &["src", "first"]),
            module("workspace:tools", &["src", "second"]),
        ],
    ));

    let target = compiler.compile(&unit).unwrap();
    let built =
        super::build_executables(NativeImageSetCompileRequest::all(target), &package_root).unwrap();

    assert_eq!(
        built
            .entries()
            .iter()
            .map(|entry| entry.identity().name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    for name in ["first", "second"] {
        assert_eq!(
            &fs::read(package_root.join(name)).unwrap()[..4],
            &[0xcf, 0xfa, 0xed, 0xfe]
        );
    }
    assert_eq!(temporary_entries(&package_root), Vec::<String>::new());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_build_rejects_cross_root_output_collisions_before_writing() {
    let directory = unique_test_directory("build-collision");
    let first_root = directory.join("first-package");
    let second_root = directory.join("second-package");
    for (root, name) in [(&first_root, "first"), (&second_root, "second")] {
        write_source(
            root,
            "index.nct",
            &format!(
                "#package: {{ name: \"{name}\", version: \"0.0.0\", }}\n#executable: {{ name: \"tool\" }}\n"
            ),
        );
        write_source(root, "index.nct", "func main(): void { return }\n");
    }
    let (mut compiler, unit) = command_discover(package_request(
        vec![
            package("workspace:first", "first", &first_root),
            package("workspace:second", "second", &second_root),
        ],
        vec![
            module("workspace:first", &[]),
            module("workspace:second", &[]),
        ],
    ));

    let target = compiler.compile(&unit).unwrap();
    let error = super::build_executables(NativeImageSetCompileRequest::all(target), &directory)
        .unwrap_err();

    assert!(matches!(error, super::BuildSetCommandError::Plan(_)));
    assert!(!directory.join("tool").exists());
    assert_eq!(temporary_entries(&directory), Vec::<String>::new());
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
#[test]
fn run_returns_a_nonzero_program_status_without_classifying_it_as_orchestration_failure() {
    let directory = unique_test_directory("run-status");
    let source = directory.join("status.nct");
    fs::write(&source, "func main(): i32 { 7 }\n").unwrap();
    let (mut compiler, unit) = command_discover(single_file_request(&source));

    let target = compiler.compile(&unit).unwrap();
    let executed = super::run_executable(
        ExecutableCompileRequest::only(target),
        &directory,
        super::RunProgramArguments::default(),
    )
    .unwrap();

    assert_eq!(executed.status().code(), Some(7));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn parsed_package_build_crosses_exact_resolution_discovery_and_publication() {
    let directory = unique_test_directory("prepared-package-build");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#executable: { name: \"hello\" }\n",
    );
    write_source(&package_root, "index.nct", "func main(): void { return }\n");
    let super::ParsedCommand::Build(parsed) = super::parse_command_arguments([
        "build".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected build command");
    };
    let prepared = parsed.prepare(&directory).unwrap();

    let result =
        super::execute_prepared_build(prepared, &command_toolchain(), &mut NoRemoteAcquisition)
            .unwrap();

    let super::BuildCommandResult::PackageSet(result) = result else {
        panic!("default package build must produce the package set");
    };
    assert_eq!(result.entries().len(), 1);
    assert_eq!(result.entries()[0].identity().name(), "hello");
    assert_eq!(
        &fs::read(package_root.join("hello")).unwrap()[..4],
        &[0xcf, 0xfa, 0xed, 0xfe]
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn named_package_build_does_not_expand_an_unselected_target_module() {
    let directory = unique_test_directory("prepared-named-build");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#executable: { name: \"good\", module: \"./src/good\" }\n#executable: { name: \"broken\", module: \"./src/broken\" }\n",
    );
    write_source(&package_root, "index.nct", "//! Package root.\n");
    write_source(
        &package_root,
        "src/good/index.nct",
        "func main(): void { return }\n",
    );
    write_source(&package_root, "src/broken/index.nct", "func main(");
    let super::ParsedCommand::Build(parsed) = super::parse_command_arguments([
        "build".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
        "--executable".into(),
        "good".into(),
    ])
    .unwrap() else {
        panic!("expected build command");
    };
    let prepared = parsed.prepare(&directory).unwrap();

    let result =
        super::execute_prepared_build(prepared, &command_toolchain(), &mut NoRemoteAcquisition)
            .unwrap();

    let super::BuildCommandResult::Selected(result) = result else {
        panic!("named package build must produce one selected executable");
    };
    assert_eq!(
        result.artifact().path(),
        fs::canonicalize(&package_root).unwrap().join("good")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn parsed_resolution_policy_reaches_the_exact_package_boundary() {
    let directory = unique_test_directory("prepared-package-policy");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#dependencies: { remote: { archive: \"https://example.test/package.tar.gz\" } }\n",
    );
    write_source(&package_root, "index.nct", "//! Package root.\n");
    let super::ParsedCommand::Build(parsed) = super::parse_command_arguments([
        "build".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
        "--offline".into(),
    ])
    .unwrap() else {
        panic!("expected build command");
    };
    let prepared = parsed.prepare(&directory).unwrap();

    let error =
        super::execute_prepared_build(prepared, &command_toolchain(), &mut NoRemoteAcquisition)
            .unwrap_err();

    assert!(matches!(
        error,
        super::BuildCommandExecutionError::Source(
            super::CommandSourceError::Package(
                super::CommandPackageStateError::Resolution(
                    PackageResolutionError::MissingLockOffline { ref alias, .. }
                )
            )
        ) if alias.as_ref() == "remote"
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_build_commits_graph_validated_acquisition_before_discovery() {
    let directory = unique_test_directory("prepared-package-acquisition");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#dependencies: { remote: { archive: \"https://example.test/package.tar.gz\" } }\n#executable: { name: \"hello\" }\n",
    );
    write_source(&package_root, "index.nct", "func main(): void { return }\n");
    let super::ParsedCommand::Build(parsed) = super::parse_command_arguments([
        "build".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected build command");
    };
    let prepared = parsed.prepare(&directory).unwrap();
    let mut acquisition = FixtureAcquisition::default();

    let result =
        super::execute_prepared_build(prepared, &command_toolchain(), &mut acquisition).unwrap();

    assert!(matches!(result, super::BuildCommandResult::PackageSet(_)));
    assert_eq!(acquisition.lock_calls, 1);
    assert_eq!(acquisition.fetch_calls, 1);
    assert!(
        package_root
            .join(format!(
                ".nocter/packages/sha256-{ARCHIVE_DIGEST}/index.nct"
            ))
            .is_file()
    );
    let root_source = fs::read_to_string(package_root.join("index.nct")).unwrap();
    assert!(root_source.contains(&format!("sha256: \"{ARCHIVE_DIGEST}\"")));
    assert!(package_root.join("hello").is_file());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn fetch_commits_the_shared_graph_validated_package_transaction_without_compiling() {
    let directory = unique_test_directory("prepared-package-fetch");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#dependencies: { remote: { archive: \"https://example.test/package.tar.gz\" } }\n",
    );
    let super::ParsedCommand::Fetch(parsed) = super::parse_command_arguments([
        "fetch".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected fetch command");
    };
    let prepared = parsed.prepare(&directory).unwrap();
    let mut acquisition = FixtureAcquisition::default();

    let toolchain = command_toolchain();
    let result =
        super::execute_prepared_fetch(prepared, toolchain.packages(), &mut acquisition).unwrap();

    assert_eq!(
        result.root(),
        &nocter_package::PackageId::from_canonical_path(&fs::canonicalize(&package_root).unwrap())
            .unwrap()
            .package_identity()
    );
    assert_eq!(acquisition.lock_calls, 1);
    assert_eq!(acquisition.fetch_calls, 1);
    assert!(
        package_root
            .join(format!(
                ".nocter/packages/sha256-{ARCHIVE_DIGEST}/index.nct"
            ))
            .is_file()
    );
    let root_source = fs::read_to_string(package_root.join("index.nct")).unwrap();
    assert!(root_source.contains(&format!("sha256: \"{ARCHIVE_DIGEST}\"")));
    let mut root_entries = fs::read_dir(&package_root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    root_entries.sort();
    assert_eq!(
        root_entries,
        [std::ffi::OsString::from(".nocter"), "index.nct".into()]
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn check_accepts_library_packages_and_single_files_without_emitting_artifacts() {
    let directory = unique_test_directory("prepared-check");
    let package_root = directory.join("library");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"library\", version: \"0.0.0\", }\n",
    );
    write_source(&package_root, "index.nct", "//! Library root.\n");
    write_source(
        &directory,
        "application.nct",
        "func main(): i32 { return 0 }\n",
    );

    let super::ParsedCommand::Check(package) = super::parse_command_arguments([
        "check".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected check command");
    };
    super::execute_prepared_check(
        package.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();

    let super::ParsedCommand::Check(file) =
        super::parse_command_arguments(["check".into(), "application.nct".into()]).unwrap()
    else {
        panic!("expected check command");
    };
    super::execute_prepared_check(
        file.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();

    let mut entries = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        [
            std::ffi::OsString::from("application.nct"),
            "library".into()
        ]
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn named_check_selects_exactly_one_module_and_rejects_an_unknown_target() {
    let directory = unique_test_directory("prepared-named-check");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"application\", version: \"0.0.0\", }\n#executable: { name: \"good\", module: \"./src/good\" }\n#executable: { name: \"broken\", module: \"./src/broken\" }\n",
    );
    write_source(&package_root, "index.nct", "//! Package root.\n");
    write_source(
        &package_root,
        "src/good/index.nct",
        "func main(): void { return }\n",
    );
    write_source(&package_root, "src/broken/index.nct", "func main(");

    let prepare = |name: &str| {
        let super::ParsedCommand::Check(parsed) = super::parse_command_arguments([
            "check".into(),
            "--root".into(),
            package_root.as_os_str().to_owned(),
            "--executable".into(),
            name.into(),
        ])
        .unwrap() else {
            panic!("expected check command");
        };
        parsed.prepare(&directory).unwrap()
    };

    super::execute_prepared_check(
        prepare("good"),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();
    let error = super::execute_prepared_check(
        prepare("missing"),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        super::CheckCommandExecutionError::Source {
            error,
            ..
        } if matches!(*error, super::CommandSourceError::MissingCommandExecutable { ref name, .. } if name.as_ref() == "missing")
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn parsed_single_file_run_forwards_program_arguments_across_the_command_adapter() {
    let directory = unique_test_directory("prepared-file-run");
    write_source(
        &directory,
        "status.nct",
        "use std/process\n\
         func main(): i32! {\n\
             if process.arg_count() != 3 { return 10 }\n\
             let first: &str = process.arg(1)? otherwise { return 11 }\n\
             if first != \"--target\" { return 12 }\n\
             let second: &str = process.arg(2)? otherwise { return 13 }\n\
             if second != \"two words\" { return 14 }\n\
             return 0\n\
         }\n",
    );
    let super::ParsedCommand::Run(parsed) = super::parse_command_arguments([
        "run".into(),
        "status.nct".into(),
        "--".into(),
        "--target".into(),
        "two words".into(),
    ])
    .unwrap() else {
        panic!("expected run command");
    };
    let prepared = parsed.prepare(&directory).unwrap();

    let executed =
        super::execute_prepared_run(prepared, &command_toolchain(), &mut NoRemoteAcquisition)
            .unwrap();

    assert_eq!(executed.status().code(), Some(0));
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(all(unix, target_arch = "aarch64", target_os = "macos"))]
#[test]
fn parsed_run_forwards_a_non_unicode_program_argument_without_decoding_it() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let directory = unique_test_directory("prepared-native-argument-run");
    write_source(
        &directory,
        "status.nct",
        "use std/process\n\
         func main(): i32 {\n\
             if process.arg_count() == 2 { return 0 }\n\
             return 1\n\
         }\n",
    );
    let super::ParsedCommand::Run(parsed) = super::parse_command_arguments([
        OsString::from("run"),
        OsString::from("status.nct"),
        OsString::from("--"),
        OsString::from_vec(vec![0xff, b'a']),
    ])
    .unwrap() else {
        panic!("expected run command");
    };

    let executed = super::execute_prepared_run(
        parsed.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();

    assert_eq!(executed.status().code(), Some(0));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn every_public_single_file_example_runs_to_success() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let examples = compiler_root.join("../../examples");
    let mut sources = fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nct"))
        .collect::<Vec<_>>();
    sources.sort();
    assert!(!sources.is_empty());

    for source in sources {
        let (mut compiler, unit) = command_discover(single_file_request(&source));
        let output_directory = unique_test_directory("public-example");
        let executable = output_directory.join("program");
        let target_program = compiler
            .compile(&unit)
            .unwrap_or_else(|error| panic!("{} failed to analyze: {error:?}", source.display()));
        super::build_executable(ExecutableCompileRequest::only(target_program), &executable)
            .unwrap_or_else(|error| panic!("{} failed to build: {error:?}", source.display()));
        let executed = Command::new(&executable)
            .current_dir(&examples)
            .output()
            .unwrap_or_else(|error| panic!("{} failed to launch: {error:?}", source.display()));
        let name = source.file_name().unwrap().to_str().unwrap();
        assert!(
            executed.status.success(),
            "{} exited with {:?}",
            source.display(),
            executed.status.code()
        );
        assert_eq!(
            executed.stdout,
            expected_example_output(name),
            "unexpected output from {}",
            source.display()
        );
        assert!(
            executed.stderr.is_empty(),
            "unexpected stderr from {}",
            source.display()
        );
        fs::remove_dir_all(output_directory).unwrap();
    }
}

#[test]
fn standard_text_output_selects_stdout_and_stderr_symmetrically() {
    let directory = unique_test_directory("standard-text-output");
    write_source(
        &directory,
        "output.nct",
        concat!(
            "use std/io\n\n",
            "func main(): i32! {\n",
            "    io.print(\"out\")?\n",
            "    io.println(\"-line\")?\n",
            "    io.println(\"\")?\n",
            "    io.eprint(\"err\")?\n",
            "    io.eprintln(\"-line\")?\n",
            "    io.eprintln(\"\")?\n",
            "    return 0\n",
            "}\n",
        ),
    );
    let source = directory.join("output.nct");
    let (mut compiler, unit) = command_discover(single_file_request(&source));
    let executable = directory.join("output");
    let target = compiler.compile(&unit).unwrap();
    super::build_executable(ExecutableCompileRequest::only(target), &executable).unwrap();

    let executed = Command::new(&executable).output().unwrap();
    assert_eq!(executed.status.code(), Some(0));
    assert_eq!(executed.stdout, b"out-line\n\n");
    assert_eq!(executed.stderr, b"err-line\n\n");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn bundled_standard_error_runtime_crosses_public_apis_and_native_cleanup() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = compiler_root.join("tests/fixtures/standard/error-runtime.nct");
    let (mut compiler, unit) = command_discover(single_file_request(&source));
    let output_directory = unique_test_directory("standard-error-runtime");
    let executable = output_directory.join("program");
    let target_program = compiler.compile(&unit).unwrap();
    super::build_executable(ExecutableCompileRequest::only(target_program), &executable).unwrap();

    let verified = Command::new(&executable).output().unwrap();
    assert_eq!(verified.status.code(), Some(0));
    assert!(verified.stdout.is_empty());
    assert!(verified.stderr.is_empty());

    let reported = Command::new(&executable).arg("report").output().unwrap();
    assert_eq!(reported.status.code(), Some(1));
    assert!(reported.stdout.is_empty());
    assert_eq!(reported.stderr, b"phase1.report: outer: inner: leaf\n");

    fs::remove_dir_all(output_directory).unwrap();
}

#[test]
fn bundled_standard_filesystem_runtime_crosses_public_stream_and_os_contracts() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = compiler_root.join("tests/fixtures/standard/filesystem-runtime.nct");
    let (mut compiler, unit) = command_discover(single_file_request(&source));
    let output_directory = unique_test_directory("standard-filesystem-runtime");
    let executable = output_directory.join("program");
    let target_program = compiler.compile(&unit).unwrap_or_else(|error| {
        let sources = unit
            .sources()
            .iter()
            .map(|source| (source.id(), source.name().as_str()))
            .collect::<Vec<_>>();
        panic!("filesystem runtime failed to analyze: {error:?}\nsources: {sources:#?}")
    });
    super::build_executable(ExecutableCompileRequest::only(target_program), &executable)
        .unwrap_or_else(|error| {
            let sources = unit
                .sources()
                .iter()
                .map(|source| (source.id(), source.name().as_str()))
                .collect::<Vec<_>>();
            panic!("filesystem runtime failed to compile: {error:?}\nsources: {sources:#?}")
        });

    let link_target = output_directory.join("link-target.txt");
    let link = output_directory.join("link.txt");
    fs::write(&link_target, b"hello").unwrap();
    std::os::unix::fs::symlink(&link_target, &link).unwrap();

    let executed = Command::new(&executable)
        .arg(&output_directory)
        .output()
        .unwrap();
    assert_eq!(executed.status.code(), Some(0));
    assert!(executed.stdout.is_empty());
    assert!(executed.stderr.is_empty());
    assert!(!output_directory.join("text.txt").exists());
    assert!(!output_directory.join("renamed.txt").exists());
    assert!(!output_directory.join("binary.dat").exists());
    assert!(!link.exists());
    assert!(!link_target.exists());

    fs::remove_dir_all(output_directory).unwrap();
}

fn expected_example_output(name: &str) -> &'static [u8] {
    match name {
        "custom-format.nct" => b"point = (3, 4)\n",
        "elapsed.nct" => b"at least two milliseconds elapsed\n",
        "equality.nct" => b"equality found the point\n",
        "hello.nct" => b"Hello from Nocter\n",
        "indexing.nct" | "recovery.nct" => b"",
        "mutable-iteration.nct" => b"mutable iteration updated every element\n",
        "ordering.nct" => b"strict ordering selected source declarations\n",
        _ => panic!("public example has no output contract: {name}"),
    }
}

#[test]
fn every_public_package_example_runs_with_its_process_contract() {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for contract in PUBLIC_PACKAGE_EXAMPLES {
        run_public_package_example(&compiler_root, *contract);
    }
}

fn run_public_package_example(compiler_root: &Path, contract: PublicPackageExample) {
    let package_root = compiler_root
        .join("../../examples")
        .join(contract.directory());
    let (mut compiler, unit) = command_discover(package_request(
        vec![package(
            contract.package_identity(),
            contract.executable(),
            &package_root,
        )],
        vec![module(contract.package_identity(), &[])],
    ));
    let output_directory = unique_test_directory("public-package-example");
    materialize_public_example_fixtures(&output_directory, contract.fixtures());
    let executable = output_directory.join(contract.executable());
    let target_program = compiler.compile(&unit).unwrap();
    super::build_executable(
        ExecutableCompileRequest::named(target_program, contract.executable()),
        &executable,
    )
    .unwrap_or_else(|error| panic!("{} failed to build: {error:?}", contract.directory()));

    for run in contract.runs() {
        let mut command = Command::new(&executable);
        command.current_dir(&output_directory);
        for argument in run.arguments() {
            match argument {
                PublicExampleArgument::FixturePath(path) => {
                    assert_fixture_relative_path(path);
                    command.arg(path);
                }
                PublicExampleArgument::Text(value) => {
                    command.arg(value);
                }
            }
        }
        let executed = command.output().unwrap_or_else(|error| {
            panic!(
                "{} {} failed to launch: {error:?}",
                contract.directory(),
                run.name()
            )
        });

        assert_eq!(
            executed.status.code(),
            Some(run.status()),
            "unexpected status from {} {}",
            contract.directory(),
            run.name()
        );
        assert_eq!(
            executed.stdout,
            run.stdout(),
            "unexpected stdout from {} {}",
            contract.directory(),
            run.name()
        );
        assert_eq!(
            executed.stderr,
            run.stderr(),
            "unexpected stderr from {} {}",
            contract.directory(),
            run.name()
        );
    }
    fs::remove_dir_all(output_directory).unwrap();
}

fn materialize_public_example_fixtures(root: &Path, fixtures: &[PublicExampleFixture]) {
    for fixture in fixtures {
        match fixture {
            PublicExampleFixture::File { path, contents } => {
                let destination = fixture_destination(root, path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(destination, contents).unwrap();
            }
            PublicExampleFixture::Directory { path } => {
                fs::create_dir_all(fixture_destination(root, path)).unwrap();
            }
            PublicExampleFixture::Symlink { path, target } => {
                let destination = fixture_destination(root, path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, destination).unwrap();
                #[cfg(not(unix))]
                panic!("public example symlink fixtures require a Unix host");
            }
        }
    }
}

fn fixture_destination(root: &Path, relative: &str) -> PathBuf {
    assert_fixture_relative_path(relative);
    root.join(relative)
}

fn assert_fixture_relative_path(relative: &str) {
    use std::path::Component;

    let path = Path::new(relative);
    assert!(!relative.is_empty(), "fixture path must not be empty");
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "fixture path must remain below its temporary root: {relative}"
    );
}

#[test]
fn parsed_package_test_runs_every_case_independently_and_preserves_failures() {
    let directory = unique_test_directory("prepared-test");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"tested\", version: \"0.0.0\", }\n#test: { name: \"unit\", module: \".\" }\n",
    );
    write_source(
        &package_root,
        "index.nct",
        "test passes { return }\ntest fails { return error.new(\"tested.failure\", \"failed\") }\n",
    );
    let super::ParsedCommand::Test(case_without_target) = super::parse_command_arguments([
        "test".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
        "--case".into(),
        "passes".into(),
    ])
    .unwrap() else {
        panic!("expected test command");
    };
    assert!(matches!(
        case_without_target.prepare(&directory),
        Err(super::PreparedCommandError::Plan(
            super::CommandPlanError::CaseWithoutTest
        ))
    ));
    let super::ParsedCommand::Test(parsed) = super::parse_command_arguments([
        "test".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected test command");
    };

    let result = super::execute_prepared_test(
        parsed.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();

    assert!(!result.succeeded());
    assert_eq!(result.summary().passed(), 1);
    assert_eq!(result.summary().failed(), 1);
    assert_eq!(
        result
            .runs()
            .iter()
            .map(|run| (
                run.target().name(),
                run.test().unwrap().name(),
                run.outcome(),
                run.exit_code(),
            ))
            .collect::<Vec<_>>(),
        [
            ("unit", "passes", super::TestRunOutcome::Passed, Some(0)),
            ("unit", "fails", super::TestRunOutcome::Failed, Some(1)),
        ]
    );
    assert_eq!(result.runs()[1].stdout(), b"");
    assert_eq!(result.runs()[1].stderr(), b"tested.failure: failed\n");

    let super::ParsedCommand::Test(parsed) = super::parse_command_arguments([
        "test".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
        "--test".into(),
        "unit".into(),
        "--case".into(),
        "passes".into(),
    ])
    .unwrap() else {
        panic!("expected test command");
    };
    let selected = super::execute_prepared_test(
        parsed.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();
    assert_eq!(selected.runs().len(), 1);
    assert_eq!(selected.runs()[0].test().unwrap().name(), "passes");
    assert!(selected.succeeded());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn one_invalid_test_target_does_not_suppress_later_target_sessions() {
    let directory = unique_test_directory("isolated-test-targets");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "index.nct",
        "#package: { name: \"isolated\", version: \"0.0.0\", }\n#test: { name: \"broken\", module: \"./broken\" }\n#test: { name: \"good\", module: \"./good\" }\n",
    );
    write_source(&package_root, "index.nct", "//! Isolated tests.\n");
    write_source(&package_root, "broken/index.nct", "test incomplete {");
    write_source(&package_root, "good/index.nct", "test passes { return }\n");
    let super::ParsedCommand::Test(parsed) = super::parse_command_arguments([
        "test".into(),
        "--root".into(),
        package_root.as_os_str().to_owned(),
    ])
    .unwrap() else {
        panic!("expected test command");
    };

    let result = super::execute_prepared_test(
        parsed.prepare(&directory).unwrap(),
        &command_toolchain(),
        &mut NoRemoteAcquisition,
    )
    .unwrap();

    assert_eq!(result.summary().passed(), 1);
    assert_eq!(result.summary().failed(), 1);
    assert_eq!(result.runs()[0].target().name(), "broken");
    assert_eq!(result.runs()[0].test(), None);
    assert_eq!(
        result.runs()[0].outcome(),
        super::TestRunOutcome::CompileFailed
    );
    assert!(!result.runs()[0].source_diagnostics().is_empty());
    assert!(result.runs()[0].sources().is_some());
    assert_eq!(result.runs()[1].target().name(), "good");
    assert_eq!(result.runs()[1].test_name(), Some("passes"));
    assert_eq!(result.runs()[1].outcome(), super::TestRunOutcome::Passed);
    fs::remove_dir_all(directory).unwrap();
}

fn command_toolchain() -> super::CommandToolchain {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    super::CommandToolchain::new(
        CompilationTarget::Arm64Darwin,
        compiler_root.join("../packaging"),
        StandardPackage::new(
            PackageIdentity::new("toolchain:std"),
            compiler_root.join("../std"),
            repository_release_version(),
        ),
    )
}

fn hello_request() -> DiscoveryRequest {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = compiler_root.join("../../examples/hello.nct");
    let standard_root = compiler_root.join("../std");
    let package = PackageIdentity::new("toolchain:std");
    DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        source,
        package_graph(vec![resolved_standard(&standard_root, &package)]),
        bundled_standard_toolchain(&package),
    )
}

fn single_file_request(source: &Path) -> DiscoveryRequest {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package = PackageIdentity::new("toolchain:std");
    DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        source,
        package_graph(vec![resolved_standard(&standard_root, &package)]),
        bundled_standard_toolchain(&package),
    )
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(package.clone(), root).with_standard_dependency(package.clone())
}

fn package(identity: &str, _name: &str, root: &Path) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(PackageIdentity::new(identity), root)
        .with_standard_dependency(PackageIdentity::new("toolchain:std"))
}

fn module(package: &str, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied())
}

fn package_request(
    mut packages: Vec<ResolvedPackageSpec>,
    roots: Vec<ModuleIdentity>,
) -> DiscoveryRequest {
    let compiler_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let standard = PackageIdentity::new("toolchain:std");
    packages.push(resolved_standard(&standard_root, &standard));
    DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(packages),
        roots,
        bundled_standard_toolchain(&standard),
    )
}

fn command_discover(
    request: DiscoveryRequest,
) -> (
    super::compiler::CommandCompiler,
    nocter_compiler_computation::CompilerDiscoveredUnit,
) {
    let mut compiler = super::compiler::CommandCompiler::default();
    let unit = compiler.discover(request).unwrap();
    (compiler, unit)
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load_with_root_catalog(
        packages,
        nocter_package::PackageRootCatalog::new(nocter_filesystem::SourceOverlay::empty()),
        &mut nocter_syntax::DirectSourceSyntax,
    )
    .unwrap()
}

fn write_source(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    contents.push_str(text);
    fs::write(path, contents).unwrap();
}

fn unique_test_directory(label: &str) -> std::path::PathBuf {
    loop {
        let directory = std::env::temp_dir().join(format!(
            "nocter-command-test-{label}-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&directory) {
            Ok(()) => return directory,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("failed to create test directory: {error}"),
        }
    }
}

fn temporary_entries(directory: &std::path::Path) -> Vec<String> {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".nocter-"))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}
