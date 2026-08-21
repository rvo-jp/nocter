use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_model::CompilationTarget;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_session::{
    ExecutableCompileRequest, ExecutableSelector, NativeImageSetCompileRequest,
    bundled_standard_toolchain,
};

use super::artifact::persist_bytes;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

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
    fs::write(directory.join("nocter.nct"), "#name: \"app\"\n").unwrap();

    let selected = super::resolve_program_input(
        &directory,
        super::ProgramInputOptions::package(None::<PathBuf>),
    )
    .unwrap();

    let package = selected.package().unwrap();
    assert_eq!(package.root(), fs::canonicalize(&directory).unwrap());
    assert_eq!(package.declaration(), package.root().join("nocter.nct"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_input_never_searches_an_ancestor_for_nocter_nct() {
    let directory = unique_test_directory("input-no-ancestor");
    fs::write(directory.join("nocter.nct"), "#name: \"parent\"\n").unwrap();
    let child = directory.join("child");
    fs::create_dir(&child).unwrap();

    let error =
        super::resolve_program_input(&child, super::ProgramInputOptions::package(None::<PathBuf>))
            .unwrap_err();

    assert!(matches!(
        error,
        super::ProgramInputError::MissingPackageDeclaration(path)
            if path == fs::canonicalize(&child).unwrap().join("nocter.nct")
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

    assert_eq!(positional, explicit);
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
    fs::write(package_root.join("nocter.nct"), "#name: \"app\"\n").unwrap();
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

    let run = super::RunCommandPlan::new(package(), super::RunCommandOptions::executable("tool"))
        .unwrap();
    assert!(matches!(run.selector(), ExecutableSelector::Named(name) if name.as_ref() == "tool"));
    assert_eq!(run.working_directory(), canonical_package);

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
        super::RunCommandPlan::new(file(), super::RunCommandOptions::executable("forbidden"))
            .unwrap_err(),
        super::CommandPlanError::ExecutableWithSingleFile
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn build_commits_one_complete_session_image_at_the_requested_path() {
    let unit = discover_hello();
    let directory = unique_test_directory("build");
    let output = directory.join("hello");

    let built = super::build_executable(ExecutableCompileRequest::only(&unit), &output).unwrap();

    assert_eq!(built.artifact().path(), output);
    assert_eq!(&fs::read(&output).unwrap()[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
    assert_eq!(temporary_entries(&directory), Vec::<String>::new());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_build_publishes_all_executables_in_declaration_order() {
    let directory = unique_test_directory("build-set");
    let package_root = directory.join("package");
    write_source(
        &package_root,
        "nocter.nct",
        "#name: \"tools\"\n#executable: { name: \"first\", module: \"./src/first\" }\n#executable: { name: \"second\", module: \"./src/second\" }\n",
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
    let unit = discover_package(
        vec![package("workspace:tools", "tools", &package_root)],
        vec![
            module("workspace:tools", &[]),
            module("workspace:tools", &["src", "first"]),
            module("workspace:tools", &["src", "second"]),
        ],
    );

    let built =
        super::build_executables(NativeImageSetCompileRequest::all(&unit), &package_root).unwrap();

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
            "nocter.nct",
            &format!("#name: \"{name}\"\n#executable: {{ name: \"tool\" }}\n"),
        );
        write_source(root, "index.nct", "func main(): void { return }\n");
    }
    let unit = discover_package(
        vec![
            package("workspace:first", "first", &first_root),
            package("workspace:second", "second", &second_root),
        ],
        vec![
            module("workspace:first", &[]),
            module("workspace:second", &[]),
        ],
    );

    let error =
        super::build_executables(NativeImageSetCompileRequest::all(&unit), &directory).unwrap_err();

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
    let unit = discover_single_file(&source);

    let executed =
        super::run_executable(ExecutableCompileRequest::only(&unit), &directory).unwrap();

    assert_eq!(executed.status().code(), Some(7));
    fs::remove_dir_all(directory).unwrap();
}

fn discover_hello() -> nocter_discovery::DiscoveredUnit {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = compiler.join("../../examples/hello.nct");
    discover_single_file(&source)
}

fn discover_single_file(source: &Path) -> nocter_discovery::DiscoveredUnit {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let package = PackageIdentity::new("toolchain:std");
    discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        source,
        package_graph(vec![resolved_standard(&standard_root, &package)]),
        bundled_standard_toolchain(&package),
    ))
    .unwrap()
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

fn discover_package(
    mut packages: Vec<ResolvedPackageSpec>,
    roots: Vec<ModuleIdentity>,
) -> nocter_discovery::DiscoveredUnit {
    let compiler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler.join("../std");
    let standard = PackageIdentity::new("toolchain:std");
    packages.push(resolved_standard(&standard_root, &standard));
    discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(packages),
        roots,
        bundled_standard_toolchain(&standard),
    ))
    .unwrap()
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load(packages).unwrap()
}

fn write_source(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
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
