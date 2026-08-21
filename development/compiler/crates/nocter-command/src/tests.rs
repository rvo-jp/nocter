use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::PackageIdentity;
use nocter_discovery::{DiscoveryRequest, ResolvedPackage, discover};
use nocter_model::CompilationTarget;
use nocter_session::{ExecutableCompileRequest, bundled_standard_toolchain};

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
        vec![resolved_standard(&standard_root, &package)],
        bundled_standard_toolchain(&package),
    ))
    .unwrap()
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackage {
    ResolvedPackage::new(package.clone(), "std", root).with_dependency("std", package.clone())
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
