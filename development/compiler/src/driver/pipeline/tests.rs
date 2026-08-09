use super::*;
use crate::target::DEFAULT_TARGET;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn derives_default_executable_path_from_source_path() {
    assert_eq!(
        default_executable_path(Path::new("src/app.nct")),
        PathBuf::from("src/app")
    );
}

#[test]
fn derives_fallback_executable_path_for_empty_source_path() {
    assert_eq!(
        default_executable_path(Path::new("")),
        PathBuf::from("a.out")
    );
}

#[test]
fn build_file_writes_arm64_macho_executable() {
    let root = make_temp_project("build-macho");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("app.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let bytes = fs::read(&executable).unwrap();
    assert_eq!(read_u32(&bytes, 0), 0xfeed_facf);
    assert_eq!(read_u32(&bytes, 4), 0x0100_000c);
    assert_eq!(read_u32(&bytes, 12), 0x2);
    assert!(bytes.len() > 0x4000);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&executable).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_main_return_code() {
    let root = make_temp_project("build-run");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("exit42.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return 42
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_same_file_function_call() {
    let root = make_temp_project("build-run-function-call");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("call.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 29
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(29));
}

#[test]
fn build_file_lowers_source_backed_public_function_body() {
    let root = make_temp_project("build-source-backed-function");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("index.nct");
    crate::test_files::write(
        &source,
        r#"use ./answer

pub func answer(value: i32): i32

func main(): i32 {
    return answer(40)
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("answer.nct"),
        r#"func answer(value: i32): i32 {
    return value + 2
}
"#,
    )
    .unwrap();

    let executable = root.join("source-backed");
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    assert!(fs::metadata(&executable).unwrap().len() > 0x4000);

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }
}

#[test]
fn build_file_specializes_calls_reachable_from_a_source_backed_generic_body() {
    let root = make_temp_project("build-source-backed-generic-call");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("index.nct");
    crate::test_files::write(
        &source,
        r#"use ./relay

pub func relay<T>(value: T): T

func identity<T>(value: T): T {
    return move value
}

func main(): i32 {
    return relay(42)
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("relay.nct"),
        r#"func relay<T>(value: T): T {
    return identity(move value)
}
"#,
    )
    .unwrap();

    let executable = root.join("source-backed-generic-call");
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    assert!(fs::metadata(&executable).unwrap().len() > 0x4000);

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(42));
    }
}

#[test]
fn build_file_lowers_source_backed_entry_body() {
    let root = make_temp_project("build-source-backed-entry");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("index.nct");
    crate::test_files::write(
        &source,
        r#"use ./main

pub func main(): i32
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("main.nct"),
        r#"func main(): i32 {
    return 37
}
"#,
    )
    .unwrap();

    let executable = root.join("source-backed-entry");
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    assert!(fs::metadata(&executable).unwrap().len() > 0x4000);

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let status = std::process::Command::new(&executable).status().unwrap();
        assert_eq!(status.code(), Some(37));
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_i32_function_call_with_arguments() {
    let root = make_temp_project("build-run-function-arguments");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("add.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_void_call_statement_before_return() {
    let root = make_temp_project("build-run-void-call-statement");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("effect.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    effect()
    return 42
}

func effect(): void {
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_usize_function_call_with_mixed_arguments() {
    let root = make_temp_project("build-run-usize-function-arguments");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("choose.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    let value: usize = choose(7, 42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, value: usize): usize {
    return value
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_usize_tail_call_with_argument() {
    let root = make_temp_project("build-run-usize-tail-call-argument");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("forward.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    let value: usize = forward(42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func forward(value: usize): usize {
    return identity(value)
}

func identity(value: usize): usize {
    return value
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_bool_function_call_with_mixed_arguments() {
    let root = make_temp_project("build-run-bool-function-arguments");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("choose_bool.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    if choose(7, true, 42) {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, flag: bool, size: usize): bool {
    return flag
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_str_function_call_argument() {
    let root = make_temp_project("build-run-str-function-argument");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("choose_str.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return choose("Nocter", 42)
}

func choose(name: &str, code: i32): i32 {
    return code
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_str_normal_call_result_as_argument() {
    let root = make_temp_project("build-run-str-normal-call-argument");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("str_call_argument.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return consume(title(), 42)
}

func title(): &str {
    return "Nocter"
}

func consume(name: &str, code: i32): i32 {
    return code
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_str_len_and_index_call_results() {
    let root = make_temp_project("build-run-str-call-result-ops");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("str_call_result_ops.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    let size: usize = identity("Nocter").len()
    let byte: u8 = identity("Nocter")[3]
    if size == 6 && byte == 116 {
        return 0
    } else {
        return 1
    }
}

func identity(text: &str): &str {
    return text
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_entry_success_return_code() {
    let root = make_temp_project("build-run-fallible-success");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("fallible.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32! {
    return 31
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(31));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_entry_failure() {
    let root = make_temp_project("build-run-fallible-failure");
    let nocter_home = make_nocter_home(&root);
    crate::test_files::write(
        nocter_home.join("std/error/index.nct"),
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    )
    .unwrap();
    let source = root.join("fallible_fail.nct");
    crate::test_files::write(
        &source,
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.failed: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_propagates_fallible_void_call_failure() {
    let root = make_temp_project("build-run-fallible-void-propagation");
    let nocter_home = make_nocter_home(&root);
    crate::test_files::write(
        nocter_home.join("std/error/index.nct"),
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    )
    .unwrap();
    let source = root.join("fallible_void_fail.nct");
    crate::test_files::write(
        &source,
        r#"use std/error.Error

func main(): void! {
    fail()?
}

func fail(): void! {
    return Error.new("app.inner", "inner failed")
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.inner: inner failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_i32_call_success_propagation() {
    let root = make_temp_project("build-run-fallible-i32-success-propagation");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("fallible_i32_success.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32! {
    let base = 2
    let value = answer()?
    return base + value
}

func answer(): i32! {
    return 40
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_propagates_fallible_i32_call_failure() {
    let root = make_temp_project("build-run-fallible-i32-failure-propagation");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    let source = root.join("fallible_i32_fail.nct");
    crate::test_files::write(
        &source,
        r#"use std/error.Error

func main(): i32! {
    return fail()?
}

func fail(): i32! {
    return Error.new("app.number", "number failed")
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.number: number failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_i32_catch_success() {
    let root = make_temp_project("build-run-fallible-i32-catch-success");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("fallible_i32_catch_success.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    return answer() catch error {
        return 7
    }
}

func answer(): i32! {
    return 42
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_i32_catch_failure() {
    let root = make_temp_project("build-run-fallible-i32-catch-failure");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    let source = root.join("fallible_i32_catch_failure.nct");
    crate::test_files::write(
        &source,
        r#"use std/error.Error

func main(): i32! {
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.answer: inner failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_write_text_raw_catch_failure() {
    let root = make_temp_project("build-run-write-text-raw-catch-failure");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    crate::test_files::write(
        nocter_home.join("std/io/index.nct"),
        r#"use std/error.Error

#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!

pub func fail_write(): void! {
    write_text_raw(-1, "x") catch error {
        return Error.new("app.write", error.code)
    }
    return
}
"#,
    )
    .unwrap();
    let source = root.join("write_catch_failure.nct");
    crate::test_files::write(
        &source,
        r#"use std/io.fail_write

func main(): void! {
    fail_write()?
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.write: std.io.invalid_input\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_write_bytes_raw_catch_failure() {
    let root = make_temp_project("build-run-write-bytes-raw-catch-failure");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    crate::test_files::write(
        nocter_home.join("std/string/index.nct"),
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        nocter_home.join("std/io/index.nct"),
        r#"use std/error.Error
use std/string.bytes

#target: "arm64-darwin"
pub(/) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!

pub func fail_write(): void! {
    write_bytes_raw(-1, bytes("x")) catch error {
        return Error.new("app.write", error.code)
    }
    return
}
"#,
    )
    .unwrap();
    let source = root.join("write_bytes_catch_failure.nct");
    crate::test_files::write(
        &source,
        r#"use std/io.fail_write

func main(): void! {
    fail_write()?
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.write: std.io.invalid_input\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_read_bytes_raw_catch_failure() {
    let root = make_temp_project("build-run-read-bytes-raw-catch-failure");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    crate::test_files::write(
        nocter_home.join("std/ptr/index.nct"),
        r#"pub(/) primitive from_addr<T>(address: usize): *T

pub(/) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
    )
    .unwrap();
    crate::test_files::write(
        nocter_home.join("std/io/index.nct"),
        r#"use std/error.Error
use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

#target: "arm64-darwin"
pub(/) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!

pub func fail_read(): void! {
    let buffer: &+[u8] = slice_from_raw_parts_mut(from_addr(1), 1)
    let count = read_bytes_raw(-1, buffer) catch error {
        return Error.new("app.read", error.code)
    }
    return
}
"#,
    )
    .unwrap();
    let source = root.join("read_bytes_catch_failure.nct");
    crate::test_files::write(
        &source,
        r#"use std/io.fail_read

func main(): void! {
    fail_read()?
    return
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.read: std.io.invalid_input\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_non_i32_catch_success_paths() {
    let root = make_temp_project("build-run-non-i32-catch-success");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("non_i32_catch_success.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    let left: i32 = success_left()
    let right: i32 = success_right()
    return left + right
}

func success_left(): i32 {
    let byte_status: i32 = success_byte()
    let size_status: i32 = success_size()
    return byte_status + size_status
}

func success_right(): i32 {
    let bool_status: i32 = success_bool()
    let str_status: i32 = success_str()
    let void_status: i32 = success_void()
    return bool_status + str_status + void_status
}

func success_byte(): i32 {
    let byte_value: u8 = make_byte() catch error {
        return 1
    }
    return byte_value as i32
}

func success_size(): i32 {
    let size_value: usize = make_size() catch error {
        return 2
    }
    if size_value == 8 {
        return 8
    } else {
        return 1
    }
}

func success_bool(): i32 {
    let flag_value: bool = make_flag() catch error {
        return 3
    }
    if flag_value {
        return 7
    } else {
        return 1
    }
}

func success_str(): i32 {
    let text: &str = make_text() catch error {
        return 4
    }
    return 6
}

func success_void(): i32 {
    effect() catch error {
        return 5
    }
    return 11
}

func make_byte(): u8! {
    return 10
}

func make_size(): usize! {
    return 8
}

func make_flag(): bool! {
    return true
}

func make_text(): &str! {
    return "abc"
}

func effect(): void! {
    return
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_non_i32_catch_failure_recovery_paths() {
    let root = make_temp_project("build-run-non-i32-catch-failure-recovery");
    let nocter_home = make_nocter_home(&root);
    write_std_error(&nocter_home);
    let source = root.join("non_i32_catch_failure_recovery.nct");
    crate::test_files::write(
        &source,
        r#"use std/error.Error

func main(): i32 {
    let left: i32 = recover_left()
    let right: i32 = recover_right()
    return left + right
}

func recover_left(): i32 {
    let byte_status: i32 = recover_byte()
    let size_status: i32 = recover_size()
    return byte_status + size_status
}

func recover_right(): i32 {
    let bool_status: i32 = recover_bool()
    let str_status: i32 = recover_str()
    let void_status: i32 = recover_void()
    return bool_status + str_status + void_status
}

func recover_byte(): i32 {
    let value: u8 = fail_byte() catch error {
        return 10
    }
    return value as i32
}

func recover_size(): i32 {
    let value: usize = fail_size() catch error {
        return 11
    }
    if value == 0 {
        return 0
    } else {
        return 1
    }
}

func recover_bool(): i32 {
    let value: bool = fail_flag() catch error {
        return 12
    }
    if value {
        return 0
    } else {
        return 1
    }
}

func recover_str(): i32 {
    let value: &str = fail_text() catch error {
        return 13
    }
    return 1
}

func recover_void(): i32 {
    fail_effect() catch error {
        return 14
    }
    return 1
}

func fail_byte(): u8! {
    return Error.new("app.byte", "byte failed")
}

func fail_size(): usize! {
    return Error.new("app.size", "size failed")
}

func fail_flag(): bool! {
    return Error.new("app.flag", "flag failed")
}

func fail_text(): &str! {
    return Error.new("app.text", "text failed")
}

func fail_effect(): void! {
    return Error.new("app.effect", "effect failed")
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(60));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_scalar_call_success_propagation() {
    let root = make_temp_project("build-run-fallible-scalar-success-propagation");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("fallible_scalar_success.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32! {
    let byte_value: u8 = make_byte()?
    let size_value: usize = make_size()?
    let flag_value: bool = make_flag()?
    if flag_value && size_value == 40 {
        return byte_value as i32
    } else {
        return 1
    }
}

func make_byte(): u8! {
    return 42
}

func make_size(): usize! {
    return 40
}

func make_flag(): bool! {
    return true
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_str_call_success_propagation() {
    let root = make_temp_project("build-run-fallible-str-success-propagation");
    let nocter_home = make_nocter_home(&root);
    crate::test_files::write(
        nocter_home.join("std/io/index.nct"),
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    )
    .unwrap();
    let source = root.join("fallible_str_success.nct");
    crate::test_files::write(
        &source,
        r#"use std/io.write

func main(): i32! {
    let text: &str = message()?
    write(text)?
    return 0
}

func message(): &str! {
    return "fallible text\n"
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"fallible text\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_fallible_force_unwrap_success() {
    let root = make_temp_project("build-run-fallible-force-success");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("fallible_force_success.nct");
    crate::test_files::write(
        &source,
        r#"func main(): i32 {
    let value = answer()!
    return value
}

func answer(): i32! {
    return 42
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_void_entry_with_zero_exit_code() {
    let root = make_temp_project("build-run-void");
    let nocter_home = make_nocter_home(&root);
    let source = root.join("void.nct");
    crate::test_files::write(
        &source,
        r#"func main(): void {
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let status = std::process::Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn build_file_output_runs_std_print_hello_world_through_namespace_import() {
    let root = make_temp_project("build-run-std-print-namespace");
    let nocter_home = make_nocter_home(&root);
    crate::test_files::write(
        nocter_home.join("std/io/index.nct"),
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!

pub func print(text: &str): void! {
    let marker = 1
    write_text_raw(1, text)?
    return
}
"#,
    )
    .unwrap();
    let source = root.join("hello.nct");
    crate::test_files::write(
        &source,
        r#"use std/io

func main(): void! {
    let marker = 1
    io.print("Hello, world!\n")?
}
"#,
    )
    .unwrap();

    let executable = default_executable_path(&source);
    let output =
        build_file_to_path_with_options(&source, &executable, &frontend_options(nocter_home));

    assert_diagnostics_empty(&output.diagnostics);
    assert_eq!(output.output_path, executable);
    let output = std::process::Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"Hello, world!\n");
    assert!(output.stderr.is_empty());
}

fn make_temp_project(name: &str) -> PathBuf {
    let unique = format!(
        "nocter-pipeline-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    crate::test_files::write_standard_package(&home).unwrap();
    fs::create_dir_all(home.join("std/prelude")).unwrap();
    crate::test_files::write(home.join("std/prelude/index.nct"), "").unwrap();
    write_builtin_view_surfaces(&home);
    home
}

fn write_builtin_view_surfaces(home: &Path) {
    fs::create_dir_all(home.join("std/str")).unwrap();
    fs::create_dir_all(home.join("std/slice")).unwrap();
    crate::test_files::write(
        home.join("std/str/index.nct"),
        "pub(/) primitive str_len_raw(value: &str): usize\nimpl str { pub method &self.len(): usize { return str_len_raw(self) } pub method &self.is_empty(): bool { return str_len_raw(self) == 0 } }\n",
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/slice/index.nct"),
        "pub(/) primitive slice_len_raw<T>(value: &[T]): usize\nimpl<T> [T] { pub method &self.len(): usize { return slice_len_raw(self) } pub method &self.is_empty(): bool { return slice_len_raw(self) == 0 } }\n",
    )
    .unwrap();
}

fn write_std_error(nocter_home: &Path) {
    fs::create_dir_all(nocter_home.join("std/error")).unwrap();
    crate::test_files::write(
        nocter_home.join("std/error/index.nct"),
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    )
    .unwrap();
}

fn frontend_options(nocter_home: PathBuf) -> FrontendOptions {
    FrontendOptions {
        nocter_home: Some(nocter_home),
        package_graph: None,
        target: DEFAULT_TARGET.to_string(),
    }
}

fn assert_diagnostics_empty(diagnostics: &[Diagnostic]) {
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:#?}"
    );
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut value = [0; 4];
    value.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(value)
}
