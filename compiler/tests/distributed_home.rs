use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn distributed_nocter_home_passes_doctor() {
    let output = Command::new(NOCTER)
        .arg("doctor")
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        text(&output.stdout).contains("ok"),
        "doctor stdout should contain ok:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn distributed_std_public_api_passes_check() {
    let project = TempProject::new("distributed-home-smoke");
    let source = project.write_source(
        "std_smoke.nct",
        r#"from std/fmt import append_bool, append_i32, append_str, append_string, unsupported as fmt_unsupported
from std/io import File, from_os_error, print, stderr, stdout, unsupported as io_unsupported, write_text
from std/mem import Allocator, Layout, RawBuffer, alloc, free, invalid_argument, out_of_memory, page_allocator
from std/os import OSError, OSErrorKind, Platform
from std/process import abort, args, cwd, env, exit
from std/ptr import addr, from_ref, from_ref_mut
from std/string import capacity_overflow, empty, from_str, push_str, view, with_capacity

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_io_file_methods_pass_check() {
    let project = TempProject::new("distributed-home-io-file-methods");
    let source = project.write_source(
        "io_file_methods.nct",
        r#"from std/io import File, stdout

func main(): i32! {
    let input = File.open("input.txt") catch error {
        return 0
    }
    var out = stdout()
    out.write_text("checked")?
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_success(&output);
}

#[test]
fn distributed_std_abort_builds_to_macho() {
    let project = TempProject::new("distributed-home-abort-build");
    let source = project.write_source(
        "abort_app.nct",
        r#"from std/process import abort

func main(): i32 {
    abort()
}
"#,
    );
    let executable = project.root().join("abort_app");

    let output = nocter_build(&project, &source, &executable);

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn distributed_std_sources_do_not_reintroduce_removed_placeholders() {
    let mut files = Vec::new();
    collect_nocter_sources(&distributed_home(), &mut files);

    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("make_error"),
            "`{}` still references removed make_error helper",
            file.display()
        );
        assert!(
            !text.contains(".placeholder("),
            "`{}` still contains parser-only placeholder calls",
            file.display()
        );
        assert!(
            !text.contains("primitive args_impl")
                && !text.contains("primitive env_impl")
                && !text.contains("primitive cwd_impl")
                && !text.contains("primitive error_from_os"),
            "`{}` declares a primitive outside the closed registry",
            file.display()
        );
    }
}

fn nocter_check(project: &TempProject, source: &Path) -> Output {
    Command::new(NOCTER)
        .args(["check", source.to_str().unwrap()])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap()
}

fn nocter_build(project: &TempProject, source: &Path, executable: &Path) -> Output {
    Command::new(NOCTER)
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            executable.to_str().unwrap(),
        ])
        .current_dir(project.root())
        .env("NOCTER_HOME", distributed_home())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        text(&output.stderr)
    );
}

fn assert_macho_executable(path: &Path) {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() > 4, "expected non-empty Mach-O executable");
    assert_eq!(&bytes[0..4], &[0xcf, 0xfa, 0xed, 0xfe]);
}

fn collect_nocter_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_nocter_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "nct") {
            files.push(path);
        }
    }
}

fn distributed_home() -> PathBuf {
    repo_root().join(".nocter")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live below repository root")
        .to_path_buf()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(name));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
