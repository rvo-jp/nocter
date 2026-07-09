use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_entry_exit_code() {
    let project = TempProject::new("cli-run-command");
    let source = project.write_source(
        "exit17.nct",
        r#"func main(): i32 {
    return 17
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(17),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_same_file_function_call_exit_code() {
    let project = TempProject::new("cli-run-function-call");
    let source = project.write_source(
        "call.nct",
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 13
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(13),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_function_call_with_arguments_exit_code() {
    let project = TempProject::new("cli-run-function-arguments");
    let source = project.write_source(
        "add.nct",
        r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_i32_normal_call_exit_code() {
    let project = TempProject::new("cli-run-normal-call");
    let source = project.write_source(
        "normal_call.nct",
        r#"func main(): i32 {
    let value = first(37, 5)
    return value + 5
}

func first(a: i32, b: i32): i32 {
    return a
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_reordered_i32_normal_call_exit_code() {
    let project = TempProject::new("cli-run-reordered-normal-call");
    let source = project.write_source(
        "reordered_normal_call.nct",
        r#"func main(): i32 {
    return wrapper(5, 42)
}

func wrapper(a: i32, b: i32): i32 {
    let value = second(b, a)
    return value
}

func second(a: i32, b: i32): i32 {
    return b
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(5),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_local_across_i32_normal_call_addition() {
    let project = TempProject::new("cli-run-normal-call-local-add");
    let source = project.write_source(
        "normal_call_local_add.nct",
        r#"func main(): i32 {
    let base = 5
    return base + answer()
}

func answer(): i32 {
    return 37
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_bool_inequality_exit_code() {
    let project = TempProject::new("cli-run-bool-inequality");
    let source = project.write_source(
        "bool_inequality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let enabled = ready != blocked
    if enabled {
        return 31
    } else {
        return 7
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(31),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_fallible_entry_success_exit_code() {
    let project = TempProject::new("cli-run-fallible-success");
    let source = project.write_source(
        "exit19.nct",
        r#"func main(): i32! {
    return 19
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(19),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reports_fallible_entry_failure() {
    let project = TempProject::new("cli-run-fallible-failure");
    let source = project.write_source(
        "fail.nct",
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    return make_error("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn bare_source_command_runs_source_file() {
    let project = TempProject::new("cli-run-bare-source");
    let source = project.write_source(
        "exit23.nct",
        r#"func main(): i32 {
    return 23
}
"#,
    );

    let output = nocter(&project, [source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn run_command_reports_compile_diagnostics_without_running() {
    let project = TempProject::new("cli-run-diagnostics");
    let source = project.write_source(
        "bad.nct",
        r#"func main(): i32 {
    return "bad"
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        text(&output.stdout)
    );

    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E0312]"),
        "expected return type diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("`return` value has type `str`, but function `main` returns `i32`"),
        "expected diagnostic message, got:\n{stderr}"
    );
}

#[test]
fn check_command_accepts_entry_option() {
    let project = TempProject::new("cli-check-entry");
    let source = project.write_source(
        "custom.nct",
        r#"func start(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        ["check", source.to_str().unwrap(), "--entry", "start"],
    );

    assert_eq!(output.status.code(), Some(0));
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

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    let mut command = Command::new(NOCTER);
    command
        .args(args)
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home());

    command.output().unwrap()
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

        let project = Self { root };
        project.write_nocter_home();
        project
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn nocter_home(&self) -> PathBuf {
        self.root.join(".nocter")
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn write_nocter_home(&self) {
        let home = self.nocter_home();
        fs::create_dir_all(home.join("std")).unwrap();
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
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
