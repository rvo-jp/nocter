use super::*;

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
fn build_command_lowers_ignored_scalar_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-scalar-call-statement");
    let source = project.write_source(
        "ignored_scalar_call_statement.nct",
        r#"func value(): i32 {
    return 1
}

func main(): void {
    value()
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_ignored_view_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-view-call-statement");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "ignored_view_call_statement.nct",
        r#"use std/string.bytes

func text(): &str {
    return "ignored"
}

func data(): &[u8] {
    return bytes("ignored")
}

func main(): void {
    text()
    data()
    return
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
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
