use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveredUnit, DiscoveryRequest};
use nocter_filesystem::{SourceOverlay, SourceOverride};
use nocter_model::CompilationTarget;
use nocter_model::PackageIdentity;
use nocter_package::{ResolvedPackageGraph, ResolvedPackageSpec};
use nocter_runtime_contract::{PrimitiveRole, RuntimeStorageRole};
use nocter_standard_profile::bundled_standard_toolchain;

use super::{
    NativeImage, NativeImageSetCompileRequest, NativeTestCompileRequest, NativeTestTargetOutcome,
    compile_native_image, compile_native_images, compile_native_tests,
};
use nocter_session::{AnalyzedUnit, AnalyzedUnitStatus, CompiledTarget, ExecutableCompileRequest};

mod codecs_and_callables;
mod http_server;
mod io_and_async_runtime;
mod network_and_tls;
mod recoverable_and_collections;
mod recovery_and_targets;
mod standard_and_process;
mod text_and_filesystem;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

const JSON_WRITER_CONTRACT_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/json-writer/index.nct");

const JSON_WRITER_IMPLEMENTATION_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/json-writer/implementation.nct");

const IO_WRITER_CONTRACT_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/io-writer/index.nct");

const IO_WRITER_IMPLEMENTATION_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/io-writer/implementation.nct");

const MAP_PHASE3_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/map-implementation.nct");

const INFLATE_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/inflate-runtime.nct");
const GZIP_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/gzip-runtime.nct");
const GZIP_BLOCKING_READER_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/gzip-blocking-reader-runtime.nct");
const GZIP_ASYNC_READER_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/gzip-async-reader-runtime.nct");
const TAR_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/tar-runtime.nct");
const TARGZ_BLOCKING_STREAM_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/targz-blocking-stream-runtime.nct");
const TARGZ_ASYNC_STREAM_RUNTIME_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/targz-async-stream-runtime.nct");

struct TestDiscoveredUnit {
    computation: nocter_compiler_computation::CompilerComputation,
    discovered: nocter_compiler_computation::CompilerDiscoveredUnit,
}

impl std::ops::Deref for TestDiscoveredUnit {
    type Target = DiscoveredUnit;

    fn deref(&self) -> &Self::Target {
        self.discovered.unit()
    }
}

fn discover(request: DiscoveryRequest) -> Result<TestDiscoveredUnit, Box<dyn std::error::Error>> {
    let mut computation = nocter_compiler_computation::CompilerComputation::new();
    let revision = computation.advance_sources(request.source_overlay(), 0)?;
    let discovered = computation.discover(&revision, request)?;
    Ok(TestDiscoveredUnit {
        computation,
        discovered,
    })
}

fn analyze_for_test(mut unit: TestDiscoveredUnit) -> AnalyzedUnit {
    let product = unit.computation.analyze(&unit.discovered).unwrap();
    nocter_session::analyze_unit_from_query(&product).unwrap()
}

fn compile_for_test(unit: TestDiscoveredUnit) -> CompiledTarget {
    analyze_for_test(unit).into_compilation_result().unwrap()
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn compile_single_file_native_source(
    package_root: &TempPackage,
    standard_root: &Path,
    source: &str,
) -> NativeImage {
    package_root.source("main.nct", source);
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let compiled = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();
    compiled.into_parts().0
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn byte_vector_source(bytes: &[u8]) -> String {
    let elements = bytes
        .iter()
        .map(|byte| format!("u8.truncate({byte})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Vec [{elements}]")
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const DIRECTORY_RECORD_TEST_SOURCE: &[u8] =
    include_bytes!("../../../tests/fixtures/native/session/directory-record.nct");

const COLLECTION_ORDERING_TEST_SOURCE: &str =
    include_str!("../../../tests/fixtures/native/session/collection-ordering.nct");

const PROVIDER_ASYNC_STREAM_TEST_MAIN: &str =
    include_str!("../../../tests/fixtures/native/session/provider-async-stream.nct");

const PROVIDER_LISTENER_POLICY_TEST_MAIN: &str =
    include_str!("../../../tests/fixtures/native/session/provider-listener-policy.nct");

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nocter-session-package-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn source(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn resolved_standard(root: &Path, package: &PackageIdentity) -> ResolvedPackageSpec {
    ResolvedPackageSpec::new(package.clone(), root).with_standard_dependency(package.clone())
}

fn package_graph(packages: Vec<ResolvedPackageSpec>) -> ResolvedPackageGraph {
    package_graph_with_overlay(packages, SourceOverlay::empty())
}

fn package_graph_with_overlay(
    packages: Vec<ResolvedPackageSpec>,
    overlay: SourceOverlay,
) -> ResolvedPackageGraph {
    ResolvedPackageGraph::load_with_root_catalog(
        packages,
        nocter_package::PackageRootCatalog::new(overlay),
        nocter_source::SourceIdentityDomain::new(),
        &mut nocter_syntax::DirectSourceSyntax,
    )
    .unwrap()
}

fn module_roots(root: &Path) -> Vec<Vec<Box<str>>> {
    let mut pending = vec![(root.to_path_buf(), Vec::new())];
    let mut modules = Vec::new();
    while let Some((directory, path)) = pending.pop() {
        if directory.join("index.nct").is_file() {
            modules.push(path.clone());
        }
        let mut children = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<PathBuf>>();
        children.sort();
        for child in children.into_iter().rev() {
            let mut child_path = path.clone();
            child_path.push(
                child
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
                    .into(),
            );
            pending.push((child, child_path));
        }
    }
    modules.sort();
    modules
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_native_status(image: &NativeImage, root: &Path, name: &str, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join(name);
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "native image exited with {status:?}"
    );
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_native_status(_image: &NativeImage, _root: &Path, _name: &str, _expected: i32) {}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_directory_stream(image: &NativeImage, root: &Path, expected: i32) {
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::Command;

    let executable = root.join("directory-stream");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("regular.txt"), b"text").unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    symlink("regular.txt", root.join("link")).unwrap();
    for index in 0..700 {
        fs::write(root.join(format!("batch-{index:04}")), b"").unwrap();
    }

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "directory stream exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_streaming_lines(image: &NativeImage, root: &Path, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("streaming-lines");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        root.join("lines.txt"),
        b"\nalpha\r\nlone\rbeta\n\xf0\x9f\x98\x80 split\nfinal",
    )
    .unwrap();
    fs::write(root.join("invalid.txt"), b"good\nbad\xff\nlater\n").unwrap();
    fs::write(root.join("single.txt"), b"z").unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "streaming line reader exited with {status:?}"
    );
    assert_eq!(fs::read(root.join("closed-file.txt")).unwrap(), b"");
    assert_eq!(fs::read(root.join("writer.txt")).unwrap(), b"abc");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_standard_input(image: &NativeImage, root: &Path, input: &[u8], expected: i32) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let executable = root.join("standard-input");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new(&executable)
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let status = child.wait().unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "standard input executable exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_native_test(image: &NativeImage, root: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join(name);
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "native test {name} exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-contract");
    let helper = root.join("subprocess-helper");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        b"#!/bin/sh\n[ \"$#\" -eq 2 ] || exit 21\n[ \"$1\" = \"alpha beta\" ] || exit 22\n[ \"$2\" = \"\" ] || exit 23\n[ \"$NOCTER_SUBPROCESS_TEST\" = \"inherited\" ] || exit 24\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .env("NOCTER_SUBPROCESS_TEST", "inherited")
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "subprocess contract executable exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_output_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-output-contract");
    let helper = root.join("capture-helper");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        concat!(
            "#!/bin/sh\n",
            "i=0\n",
            "while [ \"$i\" -lt 4096 ]; do\n",
            "  printf 'OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO'\n",
            "  printf 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE' >&2\n",
            "  i=$((i + 1))\n",
            "done\n",
            "printf '\\000\\377'\n",
            "printf '\\000\\376' >&2\n",
            "exit 23\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        ("empty-capture-helper", "#!/bin/sh\nexit 0\n"),
        (
            "text-capture-helper",
            "#!/bin/sh\nprintf 'hello\\n'\nprintf 'warning\\n' >&2\nexit 0\n",
        ),
        (
            "signal-capture-helper",
            "#!/bin/sh\nprintf 'signal-out'\nprintf 'signal-error' >&2\nkill -TERM $$\nexit 90\n",
        ),
        ("early-close-capture-helper", "#!/bin/sh\nexit 0\n"),
    ] {
        let path = root.join(name);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    for (name, shell) in [
        ("ordinary", None),
        ("closed-stdout", Some("exec 1>&-; exec \"$1\"")),
        ("closed-stderr", Some("exec 2>&-; exec \"$1\"")),
    ] {
        let status = match shell {
            Some(script) => Command::new("/bin/sh")
                .current_dir(root)
                .arg("-c")
                .arg(script)
                .arg("nocter-capture-test")
                .arg(&executable)
                .status()
                .unwrap(),
            None => Command::new(&executable)
                .current_dir(root)
                .status()
                .unwrap(),
        };
        assert_eq!(
            status.code(),
            Some(0),
            "subprocess output contract {name} exited with {status:?}"
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_spawned_child_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("spawned-child-contract");
    let helper = root.join("streaming-child-helper");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        b"#!/bin/sh\nIFS= read -r line || exit 31\n[ \"$line\" = \"request\" ] || exit 32\nprintf 'response\\n'\nprintf 'note\\n' >&2\n",
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "spawned child contract exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_configured_subprocess_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("configured-subprocess-contract");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    let workspace = root.join("configured-workspace");
    fs::create_dir(&workspace).unwrap();
    let environment_helper = workspace.join("environment-helper");
    fs::write(
        &environment_helper,
        concat!(
            "#!/bin/sh\n",
            "[ -f ./working-directory-marker ] || exit 31\n",
            "[ \"$KEEP\" = \"final=value\" ] || exit 32\n",
            "[ \"${REMOVE+x}\" = \"\" ] || exit 33\n",
            "[ \"${NOCTER_INHERITED+x}\" = \"\" ] || exit 34\n",
            "exit 0\n",
        ),
    )
    .unwrap();
    fs::write(workspace.join("working-directory-marker"), "ready\n").unwrap();
    fs::set_permissions(&environment_helper, fs::Permissions::from_mode(0o755)).unwrap();

    let inherited_helper = root.join("inherited-helper");
    fs::write(
        &inherited_helper,
        concat!(
            "#!/bin/sh\n",
            "[ \"$NOCTER_INHERITED\" = \"parent\" ] || exit 41\n",
            "[ \"$NOCTER_CHANGED\" = \"child=value\" ] || exit 42\n",
            "[ \"${NOCTER_REMOVED+x}\" = \"\" ] || exit 43\n",
            "exit 0\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&inherited_helper, fs::Permissions::from_mode(0o755)).unwrap();

    let transfer_helper = root.join("transfer-helper");
    fs::write(
        &transfer_helper,
        concat!(
            "#!/bin/sh\n",
            "i=0\n",
            "while [ \"$i\" -lt 2048 ]; do\n",
            "  printf 'OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO'\n",
            "  printf 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE' >&2\n",
            "  i=$((i + 1))\n",
            "done\n",
            "/bin/cat\n",
        ),
    )
    .unwrap();
    fs::set_permissions(&transfer_helper, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        (
            "empty-input-helper",
            "#!/bin/sh\nif IFS= read -r line; then exit 51; fi\nexit 0\n",
        ),
        ("early-close-helper", "#!/bin/sh\nexit 0\n"),
    ] {
        let helper = root.join(name);
        fs::write(&helper, source).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let status = Command::new(&executable)
        .current_dir(root)
        .env("NOCTER_INHERITED", "parent")
        .env("NOCTER_CHANGED", "parent")
        .env("NOCTER_REMOVED", "parent")
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "configured subprocess contract exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_lifecycle_contract(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let executable = root.join("subprocess-lifecycle-contract");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, source) in [
        ("success-helper", "#!/bin/sh\nexit 0\n"),
        ("nonzero-helper", "#!/bin/sh\nexit 23\n"),
        ("exit-127-helper", "#!/bin/sh\nexit 127\n"),
        ("signal-helper", "#!/bin/sh\nkill -TERM $$\nexit 90\n"),
        ("relative-helper", "#!/bin/sh\nexit 31\n"),
        (
            "argument-helper",
            "#!/bin/sh\n[ \"$#\" -eq 2 ] || exit 40\n[ \"$1\" = \"\" ] || exit 41\n[ \"$2\" = \"alpha beta\" ] || exit 42\nexit 0\n",
        ),
        ("terminate-helper", "#!/bin/sh\nwhile :; do :; done\n"),
        (
            "kill-helper",
            "#!/bin/sh\ntrap '' TERM\nwhile :; do :; done\n",
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let denied = root.join("denied-helper");
    fs::write(&denied, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o644)).unwrap();

    let invalid = root.join("invalid-helper");
    fs::write(&invalid, "this is not an executable image\n").unwrap();
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "subprocess lifecycle contract exited with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_subprocess_timeout_cancellation(image: &NativeImage, root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    let executable = root.join("subprocess-timeout-cancellation");
    let helper = root.join("timeout-helper");
    let pid_file = root.join("timeout-child.pid");
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &helper,
        "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nprintf 'ready\\n'\nwhile :; do :; done\n",
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

    let status = Command::new(&executable)
        .current_dir(root)
        .status()
        .unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "subprocess timeout contract exited with {status:?}"
    );
    let pid = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse::<u32>()
        .unwrap();
    let pid_argument = pid.to_string();
    let mut absent = false;
    for _ in 0..200 {
        let observed = Command::new("/bin/kill")
            .args(["-0", pid_argument.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        if !observed.success() {
            absent = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(absent, "cancelled subprocess {pid} remained observable");
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_directory_stream(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_streaming_lines(_image: &NativeImage, _root: &Path, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_standard_input(_image: &NativeImage, _root: &Path, _input: &[u8], _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_native_test(_image: &NativeImage, _root: &Path, _name: &str) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_output_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_spawned_child_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_configured_subprocess_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_lifecycle_contract(_image: &NativeImage, _root: &Path) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_subprocess_timeout_cancellation(_image: &NativeImage, _root: &Path) {}
