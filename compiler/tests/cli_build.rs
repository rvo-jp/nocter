use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn build_command_writes_default_macho_executable() {
    let project = TempProject::new("cli-build-default-output");
    let source = project.write_source(
        "app.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_entry_option() {
    let project = TempProject::new("cli-build-entry");
    let source = project.write_source(
        "custom.nct",
        r#"func start(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        ["build", source.to_str().unwrap(), "--entry", "start"],
    );
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_let_binding_return() {
    let project = TempProject::new("cli-build-let-return");
    let source = project.write_source(
        "local.nct",
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_local_addition() {
    let project = TempProject::new("cli-build-local-add");
    let source = project.write_source(
        "local_add.nct",
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_call_multiplication() {
    let project = TempProject::new("cli-build-i32-call-multiply");
    let source = project.write_source(
        "i32_call_multiply.nct",
        r#"func main(): i32 {
    return answer() * 2
}

func answer(): i32 {
    return 21
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_call_division_and_remainder() {
    let project = TempProject::new("cli-build-i32-call-div-rem");
    let source = project.write_source(
        "i32_call_div_rem.nct",
        r#"func main(): i32 {
    return total() / divisor() + dividend() % modulus()
}

func total(): i32 {
    return 60
}

func divisor(): i32 {
    return 2
}

func dividend(): i32 {
    return 25
}

func modulus(): i32 {
    return 13
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_i32_normal_call_let_initializer() {
    let project = TempProject::new("cli-build-normal-call-let");
    let source = project.write_source(
        "normal_call_let.nct",
        r#"func main(): i32 {
    let value = answer()
    return value
}

func answer(): i32 {
    return 42
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if() {
    let project = TempProject::new("cli-build-terminal-if");
    let source = project.write_source(
        "terminal_if.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 2
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_bool_local() {
    let project = TempProject::new("cli-build-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local.nct",
        r#"func main(): i32 {
    let enabled = true
    if enabled {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_bool_logical() {
    let project = TempProject::new("cli-build-terminal-if-bool-logical");
    let source = project.write_source(
        "terminal_if_bool_logical.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if ready && !blocked {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_bool_equality() {
    let project = TempProject::new("cli-build-bool-equality");
    let source = project.write_source(
        "bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_unsupported_compound_bool_equality() {
    let project = TempProject::new("cli-build-compound-bool-equality");
    let source = project.write_source(
        "compound_bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E8008]"),
        "expected IR lowering diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
        ),
        "expected bool equality/inequality operand diagnostic, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_reports_unsupported_compound_bool_equality_condition() {
    let project = TempProject::new("cli-build-compound-bool-equality-condition");
    let source = project.write_source(
        "compound_bool_equality_condition.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if !ready == blocked {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("error[E8002]"),
        "expected entry lowering diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
        ),
        "expected bool equality/inequality operand diagnostic, got:\n{stderr}"
    );
    assert!(
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[test]
fn build_command_lowers_terminal_if_i32_equality() {
    let project = TempProject::new("cli-build-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_i32_inequality() {
    let project = TempProject::new("cli-build-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_i32_less_equal() {
    let project = TempProject::new("cli-build-terminal-if-less-equal");
    let source = project.write_source(
        "terminal_if_less_equal.nct",
        r#"func main(): i32 {
    let value = 42
    if value <= 42 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_reports_compile_diagnostics_without_output() {
    let project = TempProject::new("cli-build-diagnostics");
    let source = project.write_source(
        "bad.nct",
        r#"func main(): i32 {
    return "bad"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

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
        !executable.exists(),
        "build should not leave an executable after compile diagnostics"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_entry_exit_code() {
    let project = TempProject::new("cli-build-run-exit");
    let source = project.write_source(
        "exit37.nct",
        r#"func main(): i32 {
    return 37
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(37));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_runs_same_file_i32_call_with_arguments() {
    let project = TempProject::new("cli-build-run-call-args");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_i32_let_binding_value() {
    let project = TempProject::new("cli-build-run-let-return");
    let source = project.write_source(
        "local_exit.nct",
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_i32_local_addition_value() {
    let project = TempProject::new("cli-build-run-local-add");
    let source = project.write_source(
        "local_add_exit.nct",
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_else_value() {
    let project = TempProject::new("cli-build-run-terminal-if");
    let source = project.write_source(
        "terminal_if_exit.nct",
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 42
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_equality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-equality");
    let source = project.write_source(
        "terminal_if_equality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_inequality_value() {
    let project = TempProject::new("cli-build-run-terminal-if-inequality");
    let source = project.write_source(
        "terminal_if_inequality_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value != 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_greater_value() {
    let project = TempProject::new("cli-build-run-terminal-if-greater");
    let source = project.write_source(
        "terminal_if_greater_exit.nct",
        r#"func main(): i32 {
    let value = 42
    if value > 41 {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_local_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-local");
    let source = project.write_source(
        "terminal_if_bool_local_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let enabled = true
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_terminal_if_bool_or_binding_value() {
    let project = TempProject::new("cli-build-run-terminal-if-bool-or");
    let source = project.write_source(
        "terminal_if_bool_or_exit.nct",
        r#"func main(): i32 {
    let value = 42
    let ready = false
    let fallback = true
    let enabled = ready || fallback
    if enabled {
        return value
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_fallible_entry_failure_reports_stderr() {
    let project = TempProject::new("cli-build-run-fallible-failure");
    let source = project.write_source(
        "fail.nct",
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    return make_error("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let output = Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"failed\n");
}

fn nocter<const N: usize>(project: &TempProject, args: [&str; N]) -> Output {
    Command::new(NOCTER)
        .args(args)
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
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
    assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
    assert_eq!(read_u32(&bytes, 4), 0x0100_000c);
    assert_eq!(read_u32(&bytes, 12), 0x2);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut value = [0; 4];
    value.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(value)
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
