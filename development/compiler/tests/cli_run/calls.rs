use super::*;

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
fn run_command_returns_temporary_method_receiver_exit_code() {
    let project = TempProject::new("cli-run-temporary-method-receiver");
    let source = project.write_source(
        "temporary_method_receiver.nct",
        r#"copy struct File {
    fd: i32
}

instance File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    return make_file().value()
}

func make_file(): File {
    return File { fd: 42 }
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
fn run_command_ignores_scalar_call_expression_statement() {
    let project = TempProject::new("cli-run-ignored-scalar-call-statement");
    let source = project.write_source(
        "ignored_scalar_call_statement.nct",
        r#"func main(): i32 {
    value()
    return 42
}

func value(): i32 {
    return 1
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
fn run_command_ignores_view_call_expression_statement() {
    let project = TempProject::new("cli-run-ignored-view-call-statement");
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

func main(): i32 {
    text()
    data()
    return 42
}

func text(): &str {
    return "ignored"
}

func data(): &[u8] {
    return bytes("ignored")
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
fn run_command_returns_reordered_i32_tail_call_exit_code() {
    let project = TempProject::new("cli-run-reordered-tail-call");
    let source = project.write_source(
        "reordered_tail_call.nct",
        r#"func main(): i32 {
    return wrapper(5, 42)
}

func wrapper(a: i32, b: i32): i32 {
    return second(b, a)
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
fn run_command_returns_i32_call_comparison_return_exit_code() {
    let project = TempProject::new("cli-run-i32-call-comparison-return");
    let source = project.write_source(
        "i32_call_comparison_return.nct",
        r#"func main(): i32 {
    if less() {
        return 42
    } else {
        return 7
    }
}

func less(): bool {
    return left() < right()
}

func left(): i32 {
    return 40
}

func right(): i32 {
    return 42
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
fn run_command_returns_and_i32_call_comparison_value_exit_code() {
    let project = TempProject::new("cli-run-and-i32-call-comparison-value");
    let source = project.write_source(
        "and_i32_call_comparison_value.nct",
        r#"func main(): i32 {
    let matched = answer() == 42 && ready()
    if matched {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}

func ready(): bool {
    return true
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
fn run_command_returns_multiple_i32_normal_call_addition_exit_code() {
    let project = TempProject::new("cli-run-multiple-normal-call-add");
    let source = project.write_source(
        "multiple_normal_call_add.nct",
        r#"func main(): i32 {
    return (left() + right()) + base()
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 21
}

func base(): i32 {
    return 1
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
fn run_command_returns_i32_call_arithmetic_exit_code() {
    let project = TempProject::new("cli-run-i32-call-arithmetic");
    let source = project.write_source(
        "i32_call_arithmetic.nct",
        r#"func main(): i32 {
    return answer() * 2 - offset()
}

func answer(): i32 {
    return 24
}

func offset(): i32 {
    return 6
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
fn run_command_returns_i32_call_division_and_remainder_exit_code() {
    let project = TempProject::new("cli-run-i32-call-div-rem");
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
fn run_command_returns_i32_call_shift_exit_code() {
    let project = TempProject::new("cli-run-i32-call-shift");
    let source = project.write_source(
        "i32_call_shift.nct",
        r#"func main(): i32 {
    return (value() << left_count()) + (shifted() >> right_count())
}

func value(): i32 {
    return 5
}

func left_count(): i32 {
    return 3
}

func shifted(): i32 {
    return 8
}

func right_count(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(44),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nested_i32_normal_call_argument_exit_code() {
    let project = TempProject::new("cli-run-nested-normal-call-arg");
    let source = project.write_source(
        "nested_normal_call_arg.nct",
        r#"func main(): i32 {
    let value = add(left(), right())
    return value
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
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
fn run_command_returns_nested_i32_tail_call_argument_exit_code() {
    let project = TempProject::new("cli-run-nested-tail-call-arg");
    let source = project.write_source(
        "nested_tail_call_arg.nct",
        r#"func main(): i32 {
    return add(left(), right())
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
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
fn run_command_preserves_scalar_parameter_after_normal_call() {
    let project = TempProject::new("cli-run-preserve-scalar-parameter-after-normal-call");
    let source = project.write_source(
        "preserve_scalar_parameter_after_normal_call.nct",
        r#"func main(): i32 {
    return caller(42)
}

func caller(value: i32): i32 {
    let ignored = choose(1)
    return value
}

func choose(value: i32): i32 {
    return value + 1
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
