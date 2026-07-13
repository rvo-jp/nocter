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
fn run_command_returns_imported_function_call_exit_code() {
    let project = TempProject::new("cli-run-imported-function-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "call.nct",
        r#"from std/math import answer

func main(): i32 {
    return answer()
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
fn run_command_returns_imported_alias_function_call_exit_code() {
    let project = TempProject::new("cli-run-imported-alias-function-call");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func answer(): i32 {
    return 42
}
"#,
    );
    let source = project.write_source(
        "call_alias.nct",
        r#"from std/math import answer as imported_answer

func main(): i32 {
    return imported_answer()
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
fn run_command_returns_imported_bool_condition_exit_code() {
    let project = TempProject::new("cli-run-imported-bool-condition");
    project.write_nocter_home_file(
        "std/flags.nct",
        r#"pub func ready(): bool {
    return true
}
"#,
    );
    let source = project.write_source(
        "condition.nct",
        r#"from std/flags import ready

func main(): i32 {
    if ready() {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_imported_nested_argument_exit_code() {
    let project = TempProject::new("cli-run-imported-nested-argument");
    project.write_nocter_home_file(
        "std/math.nct",
        r#"pub func base(): i32 {
    return 41
}

pub func add_one(value: i32): i32 {
    return value + 1
}
"#,
    );
    let source = project.write_source(
        "nested.nct",
        r#"from std/math import add_one
from std/math import base

func main(): i32 {
    return add_one(base())
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
fn run_command_returns_i32_normal_call_with_borrow_argument_exit_code() {
    let project = TempProject::new("cli-run-borrow-normal-call");
    let source = project.write_source(
        "borrow_arg.nct",
        r#"func main(): i32 {
    let value = 7
    let result = choose(&value, 42)
    return result
}

func choose(value: &i32, code: i32): i32 {
    return code
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
fn run_command_returns_i32_normal_call_with_readwrite_borrow_argument_exit_code() {
    let project = TempProject::new("cli-run-readwrite-borrow-normal-call");
    let source = project.write_source(
        "readwrite_borrow_arg.nct",
        r#"func main(): i32 {
    var value = 7
    let result = choose(&+value, 42)
    return result
}

func choose(value: &+i32, code: i32): i32 {
    return code
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
fn run_command_returns_scalar_var_assignment_exit_code() {
    let project = TempProject::new("cli-run-scalar-var-assignment");
    let source = project.write_source(
        "scalar_var_assignment.nct",
        r#"func main(): i32 {
    var count = 1
    count = count + 39
    var byte: u8 = 1
    byte = 2
    var size: usize = 0
    size = 40
    var flag: bool = false
    flag = ready()
    if flag && size == 40 {
        return count + (byte as i32)
    } else {
        return 1
    }
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
fn run_command_writes_reassigned_str_local() {
    let project = TempProject::new("cli-run-str-var-assignment");
    project.write_nocter_home_file(
        "std/io.nct",
        r#"from std/io_impl import write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "targets/arm64-darwin/std/io_impl.nct",
        r#"pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "str_var_assignment.nct",
        r#"from std/io import write

func main(): i32! {
    var text: &str = "wrong"
    text = "Hello"
    write(text)?
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"Hello");
    assert!(output.stderr.is_empty());
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
fn run_command_returns_usize_condition_exit_code() {
    let project = TempProject::new("cli-run-usize-condition");
    let source = project.write_source(
        "usize_condition.nct",
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 42
    } else {
        return 1
    }
}

func size(): usize {
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
fn run_command_returns_imported_usize_condition_exit_code() {
    let project = TempProject::new("cli-run-imported-usize-condition");
    project.write_nocter_home_file(
        "std/sizes.nct",
        r#"pub func size(): usize {
    return 42
}
"#,
    );
    let source = project.write_source(
        "imported_usize_condition.nct",
        r#"from std/sizes import size

func main(): i32 {
    let value: usize = size()
    if value == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_bool_normal_call_condition_exit_code() {
    let project = TempProject::new("cli-run-bool-normal-call");
    let source = project.write_source(
        "bool_normal_call.nct",
        r#"func main(): i32 {
    let value = ready()
    if value {
        return 42
    } else {
        return 7
    }
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
fn run_command_returns_not_bool_normal_call_exit_code() {
    let project = TempProject::new("cli-run-not-bool-normal-call");
    let source = project.write_source(
        "not_bool_normal_call.nct",
        r#"func main(): i32 {
    let disabled = !ready()
    if disabled {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
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
fn run_command_returns_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-bool-condition-call");
    let source = project.write_source(
        "bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() {
        return 42
    } else {
        return 7
    }
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
fn run_command_returns_not_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-not-bool-condition-call");
    let source = project.write_source(
        "not_bool_condition_call.nct",
        r#"func main(): i32 {
    if !ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
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
fn run_command_returns_and_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-and-bool-condition-call");
    let source = project.write_source(
        "and_bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() && enabled() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func enabled(): bool {
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
fn run_command_returns_or_bool_condition_call_exit_code() {
    let project = TempProject::new("cli-run-or-bool-condition-call");
    let source = project.write_source(
        "or_bool_condition_call.nct",
        r#"func main(): i32 {
    if ready() || enabled() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}

func enabled(): bool {
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
fn run_command_returns_and_bool_value_call_exit_code() {
    let project = TempProject::new("cli-run-and-bool-value-call");
    let source = project.write_source(
        "and_bool_value_call.nct",
        r#"func main(): i32 {
    let value = ready() && enabled()
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func enabled(): bool {
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
fn run_command_returns_or_bool_return_call_exit_code() {
    let project = TempProject::new("cli-run-or-bool-return-call");
    let source = project.write_source(
        "or_bool_return_call.nct",
        r#"func main(): i32 {
    if enabled() {
        return 42
    } else {
        return 7
    }
}

func enabled(): bool {
    return ready() || fallback()
}

func ready(): bool {
    return false
}

func fallback(): bool {
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
fn run_command_returns_bool_call_comparison_let_exit_code() {
    let project = TempProject::new("cli-run-bool-call-comparison-let");
    let source = project.write_source(
        "bool_call_comparison_let.nct",
        r#"func main(): i32 {
    let value = ready() == true
    if value {
        return 42
    } else {
        return 7
    }
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
fn run_command_returns_bool_call_comparison_return_exit_code() {
    let project = TempProject::new("cli-run-bool-call-comparison-return");
    let source = project.write_source(
        "bool_call_comparison_return.nct",
        r#"func main(): i32 {
    if differs() {
        return 42
    } else {
        return 7
    }
}

func differs(): bool {
    return left() != right()
}

func left(): bool {
    return true
}

func right(): bool {
    return false
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
fn run_command_returns_i32_call_comparison_condition_exit_code() {
    let project = TempProject::new("cli-run-i32-call-comparison-condition");
    let source = project.write_source(
        "i32_call_comparison_condition.nct",
        r#"func main(): i32 {
    if answer() == 42 {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
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
fn run_command_returns_and_i32_call_comparison_condition_exit_code() {
    let project = TempProject::new("cli-run-and-i32-call-comparison-condition");
    let source = project.write_source(
        "and_i32_call_comparison_condition.nct",
        r#"func main(): i32 {
    if answer() == 42 && ready() {
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
fn run_command_returns_usize_arithmetic_and_shift_exit_code() {
    let project = TempProject::new("cli-run-usize-arithmetic-shift");
    let source = project.write_source(
        "usize_arithmetic_shift.nct",
        r#"func main(): i32 {
    if combined(20, size()) == 23 {
        return 42
    } else {
        return 1
    }
}

func combined(left: usize, right: usize): usize {
    return arithmetic(left, right) + shifted_left() + shifted_right()
}

func arithmetic(left: usize, right: usize): usize {
    let doubled: usize = right * 2
    let adjusted: usize = left + doubled - 4
    let quotient: usize = adjusted / 2
    let remainder: usize = quotient % 9
    return remainder
}

func shifted_left(): usize {
    return one() << left_count()
}

func shifted_right(): usize {
    return sixty_four() >> right_count()
}

func size(): usize {
    return 6
}

func one(): usize {
    return 1
}

func sixty_four(): usize {
    return 64
}

func left_count(): usize {
    return 4
}

func right_count(): usize {
    return 5
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
fn run_command_traps_i32_division_by_zero() {
    let project = TempProject::new("cli-run-i32-div-zero");
    let source = project.write_source(
        "i32_div_zero.nct",
        r#"func main(): i32 {
    return 1 / zero()
}

func zero(): i32 {
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_signed_division_overflow() {
    let project = TempProject::new("cli-run-i32-div-overflow");
    let source = project.write_source(
        "i32_div_overflow.nct",
        r#"func main(): i32 {
    return minimum() / minus_one()
}

func minimum(): i32 {
    return -2147483648
}

func minus_one(): i32 {
    return -1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_negative_shift_count() {
    let project = TempProject::new("cli-run-i32-shift-negative");
    let source = project.write_source(
        "i32_shift_negative.nct",
        r#"func main(): i32 {
    return 1 << count()
}

func count(): i32 {
    return -1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_too_large_shift_count() {
    let project = TempProject::new("cli-run-i32-shift-too-large");
    let source = project.write_source(
        "i32_shift_too_large.nct",
        r#"func main(): i32 {
    return 1 >> count()
}

func count(): i32 {
    return 32
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_addition_overflow() {
    let project = TempProject::new("cli-run-i32-add-overflow");
    let source = project.write_source(
        "i32_add_overflow.nct",
        r#"func main(): i32 {
    return maximum() + one()
}

func maximum(): i32 {
    return 2147483647
}

func one(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_subtraction_overflow() {
    let project = TempProject::new("cli-run-i32-sub-overflow");
    let source = project.write_source(
        "i32_sub_overflow.nct",
        r#"func main(): i32 {
    return minimum() - one()
}

func minimum(): i32 {
    return -2147483648
}

func one(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_i32_multiplication_overflow() {
    let project = TempProject::new("cli-run-i32-mul-overflow");
    let source = project.write_source(
        "i32_mul_overflow.nct",
        r#"func main(): i32 {
    return maximum() * two()
}

func maximum(): i32 {
    return 2147483647
}

func two(): i32 {
    return 2
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_usize_addition_overflow() {
    let project = TempProject::new("cli-run-usize-add-overflow");
    let source = project.write_source(
        "usize_add_overflow.nct",
        r#"func main(): i32 {
    if overflow() == 0 {
        return 0
    } else {
        return 1
    }
}

func overflow(): usize {
    return maximum() + 1
}

func maximum(): usize {
    return 0xffff_ffff_ffff_ffff
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_usize_division_by_zero() {
    let project = TempProject::new("cli-run-usize-div-zero");
    let source = project.write_source(
        "usize_div_zero.nct",
        r#"func main(): i32 {
    if divide() == 0 {
        return 0
    } else {
        return 1
    }
}

func divide(): usize {
    return 1 / zero()
}

func zero(): usize {
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_traps_usize_too_large_shift_count() {
    let project = TempProject::new("cli-run-usize-shift-too-large");
    let source = project.write_source(
        "usize_shift_too_large.nct",
        r#"func main(): i32 {
    if shift() == 0 {
        return 0
    } else {
        return 1
    }
}

func shift(): usize {
    return one() << count()
}

func one(): usize {
    return 1
}

func count(): usize {
    return 64
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert!(
        !output.status.success(),
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
fn run_command_returns_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-direct-aggregate-value-arg");
    let source = project.write_source(
        "direct_aggregate_value_arg.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let result = consume(Header{ tag: 7, ok: true, code: 42, len: 11 })
    return result
}

func consume(header: Header): i32 {
    return header.code
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
fn run_command_returns_direct_aggregate_argument_between_scalars_exit_code() {
    let project = TempProject::new("cli-run-direct-aggregate-arg-between-scalars");
    let source = project.write_source(
        "direct_aggregate_arg_between_scalars.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    return consume(5, Pair{ a: 10, b: 20, c: 41, d: 2 }, 1)
}

func consume(prefix: i32, pair: Pair, suffix: i32): i32 {
    return pair.c + suffix
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
fn run_command_returns_indirect_aggregate_argument_between_scalars_exit_code() {
    let project = TempProject::new("cli-run-indirect-aggregate-arg-between-scalars");
    let source = project.write_source(
        "indirect_aggregate_arg_between_scalars.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32 {
    return consume(5, Big{ first: 1, second: 2, code: 41 }, 1)
}

func consume(prefix: i32, value: Big, suffix: i32): i32 {
    return value.code + suffix
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
fn run_command_returns_direct_aggregate_argument_at_register_boundary_exit_code() {
    let project = TempProject::new("cli-run-direct-aggregate-arg-register-boundary");
    let source = project.write_source(
        "direct_aggregate_arg_register_boundary.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, Pair{ a: 10, b: 20, c: 42, d: 7 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, pair: Pair): i32 {
    return pair.c
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
fn run_command_returns_indirect_aggregate_argument_at_register_boundary_exit_code() {
    let project = TempProject::new("cli-run-indirect-aggregate-arg-register-boundary");
    let source = project.write_source(
        "indirect_aggregate_arg_register_boundary.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, Big{ first: 10, second: 20, code: 42 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, value: Big): i32 {
    return value.code
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
fn run_command_returns_nested_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-value-arg");
    let source = project.write_source(
        "nested_aggregate_value_arg.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    let result = consume(packet.header)
    return result
}

func consume(header: Header): i32 {
    return header.code
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
fn run_command_returns_nested_aggregate_call_result_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-call-result-value-arg");
    let source = project.write_source(
        "nested_aggregate_call_result_value_arg.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let result = consume(make().header)
    return result
}

func make(): Packet {
    return Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
}

func consume(header: Header): i32 {
    return header.code
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
fn run_command_returns_nested_aggregate_fallible_call_result_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-fallible-call-result-value-arg");
    let source = project.write_source(
        "nested_aggregate_fallible_call_result_value_arg.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    return consume(make()?.header)
}

func make(): Packet! {
    return Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
}

func consume(header: Header): i32 {
    return header.code
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
fn run_command_returns_aggregate_call_binding_with_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-aggregate-call-binding-aggregate-arg");
    let source = project.write_source(
        "aggregate_call_binding_aggregate_arg.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = wrap(Header{ tag: 7, ok: true, code: 42, len: 11 })
    return packet.header.code
}

func wrap(header: Header): Packet {
    return Packet{ prefix: 1, header: header, tail: 99 }
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
fn run_command_returns_aggregate_force_unwrap_call_binding_exit_code() {
    let project = TempProject::new("cli-run-aggregate-force-unwrap-call-binding");
    let source = project.write_source(
        "aggregate_force_unwrap_call_binding.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = make()!
    return header.code
}

func make(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_aggregate_force_unwrap_value_argument_exit_code() {
    let project = TempProject::new("cli-run-aggregate-force-unwrap-value-argument");
    let source = project.write_source(
        "aggregate_force_unwrap_value_argument.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return consume(make()!)
}

func make(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
}

func consume(header: Header): i32 {
    return header.code
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
fn run_command_returns_aggregate_force_unwrap_struct_literal_field_exit_code() {
    let project = TempProject::new("cli-run-aggregate-force-unwrap-struct-literal-field");
    let source = project.write_source(
        "aggregate_force_unwrap_struct_literal_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: make()!,
        tail: 99,
    }
    return packet.header.code
}

func make(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_nested_aggregate_struct_literal_argument_call_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-struct-literal-arg-call-field");
    let source = project.write_source(
        "nested_aggregate_struct_literal_arg_call_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return consume(Packet{
        prefix: 1,
        header: make_header(),
        tail: 99,
    })
}

func make_header(): Header {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
}

func consume(packet: Packet): i32 {
    return packet.header.code
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
fn run_command_returns_direct_aggregate_struct_literal_return_call_field_exit_code() {
    let project = TempProject::new("cli-run-direct-aggregate-struct-literal-return-call-field");
    let source = project.write_source(
        "direct_aggregate_struct_literal_return_call_field.nct",
        r#"copy struct Pair {
    first: i32
    second: i32
}

copy struct Wrap {
    pair: Pair
    code: i32
}

func main(): i32 {
    let wrap = make_wrap()
    return wrap.code
}

func make_pair(): Pair {
    return Pair{ first: 1, second: 2 }
}

func make_wrap(): Wrap {
    return Wrap{ pair: make_pair(), code: 42 }
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
fn run_command_returns_small_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-small-direct-aggregate-value-arg");
    let source = project.write_source(
        "small_direct_aggregate_value_arg.nct",
        r#"struct Code {
    value: i32
}

func main(): i32 {
    let result = consume(Code{ value: 42 })
    return result
}

func consume(code: Code): i32 {
    return code.value
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
fn run_command_returns_two_byte_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-two-byte-direct-aggregate-value-arg");
    let source = project.write_source(
        "two_byte_direct_aggregate_value_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
}

func main(): i32 {
    let result = consume(Bytes{ first: 7, second: 42 })
    return result
}

func consume(bytes: Bytes): i32 {
    if bytes.second == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_three_byte_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-three-byte-direct-aggregate-value-arg");
    let source = project.write_source(
        "three_byte_direct_aggregate_value_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
}

func main(): i32 {
    let result = consume(Bytes{ first: 7, second: 11, third: 42 })
    return result
}

func consume(bytes: Bytes): i32 {
    if bytes.third == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_five_byte_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-five-byte-direct-aggregate-value-arg");
    let source = project.write_source(
        "five_byte_direct_aggregate_value_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    let result = consume(Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
    return result
}

func consume(bytes: Bytes): i32 {
    if bytes.fifth == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_shifted_five_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-five-byte-direct-aggregate-arg-between-scalars");
    let source = project.write_source(
        "five_byte_direct_aggregate_arg_between_scalars.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    return consume(5, Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 41 }, 42)
}

func consume(prefix: i32, bytes: Bytes, suffix: i32): i32 {
    if bytes.fifth == 41 {
        return suffix
    } else {
        return 1
    }
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
fn run_command_returns_boundary_five_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-five-byte-direct-aggregate-arg-register-boundary");
    let source = project.write_source(
        "five_byte_direct_aggregate_arg_register_boundary.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, bytes: Bytes): i32 {
    if bytes.fifth == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_small_direct_aggregate_call_result_field_exit_code() {
    let project = TempProject::new("cli-run-small-direct-aggregate-call-result-field");
    let source = project.write_source(
        "small_direct_aggregate_call_result_field.nct",
        r#"struct Code {
    value: i32
}

func main(): i32 {
    return make().value
}

func make(): Code {
    return Code{ value: 42 }
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
fn run_command_returns_two_byte_direct_aggregate_call_result_field_exit_code() {
    let project = TempProject::new("cli-run-two-byte-direct-aggregate-call-result-field");
    let source = project.write_source(
        "two_byte_direct_aggregate_call_result_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
}

func main(): i32 {
    if make().second == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes {
    return Bytes{ first: 7, second: 42 }
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
fn run_command_returns_six_byte_direct_aggregate_call_result_field_exit_code() {
    let project = TempProject::new("cli-run-six-byte-direct-aggregate-call-result-field");
    let source = project.write_source(
        "six_byte_direct_aggregate_call_result_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
}

func main(): i32 {
    if make().sixth == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes {
    return Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 5, sixth: 42 }
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
fn run_command_returns_seven_byte_direct_aggregate_call_result_field_exit_code() {
    let project = TempProject::new("cli-run-seven-byte-direct-aggregate-call-result-field");
    let source = project.write_source(
        "seven_byte_direct_aggregate_call_result_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
}

func main(): i32 {
    if make().seventh == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes {
    return Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 5, sixth: 6, seventh: 42 }
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
fn run_command_returns_propagated_direct_aggregate_call_return_field_exit_code() {
    let project = TempProject::new("cli-run-propagated-direct-aggregate-call-return-field");
    let source = project.write_source(
        "propagated_direct_aggregate_call_return_field.nct",
        r#"struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    var pair = forward()?
    return pair.second
}

func forward(): Pair! {
    return make()?
}

func make(): Pair! {
    return Pair{ first: 7, second: 42 }
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
fn run_command_returns_propagated_small_direct_aggregate_call_return_field_exit_code() {
    let project = TempProject::new("cli-run-propagated-small-direct-aggregate-call-return-field");
    let source = project.write_source(
        "propagated_small_direct_aggregate_call_return_field.nct",
        r#"struct Code {
    value: i32
}

func main(): i32! {
    return make()?.value
}

func make(): Code! {
    return Code{ value: 42 }
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
fn run_command_returns_propagated_five_byte_direct_aggregate_call_return_field_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-five-byte-direct-aggregate-call-return-field");
    let source = project.write_source(
        "propagated_five_byte_direct_aggregate_call_return_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32! {
    if make()?.fifth == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes! {
    return Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 42 }
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
fn run_command_returns_propagated_direct_aggregate_call_argument_field_exit_code() {
    let project = TempProject::new("cli-run-propagated-direct-aggregate-call-argument-field");
    let source = project.write_source(
        "propagated_direct_aggregate_call_argument_field.nct",
        r#"struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    return consume(make()?)
}

func make(): Pair! {
    return Pair{ first: 7, second: 42 }
}

func consume(pair: Pair): i32 {
    return pair.second
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
fn run_command_returns_propagated_direct_aggregate_call_argument_between_scalars_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-direct-aggregate-call-argument-between-scalars");
    let source = project.write_source(
        "propagated_direct_aggregate_call_argument_between_scalars.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32! {
    return consume(5, make()?, 1)
}

func make(): Pair! {
    return Pair{ a: 10, b: 20, c: 41, d: 2 }
}

func consume(prefix: i32, pair: Pair, suffix: i32): i32 {
    return pair.c + suffix
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
fn run_command_returns_propagated_indirect_aggregate_call_argument_between_scalars_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-indirect-aggregate-call-argument-between-scalars");
    let source = project.write_source(
        "propagated_indirect_aggregate_call_argument_between_scalars.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32! {
    return consume(5, make()?, 1)
}

func make(): Big! {
    return Big{ first: 1, second: 2, code: 41 }
}

func consume(prefix: i32, value: Big, suffix: i32): i32 {
    return value.code + suffix
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
fn run_command_returns_propagated_direct_aggregate_call_argument_at_register_boundary_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-direct-aggregate-call-argument-register-boundary");
    let source = project.write_source(
        "propagated_direct_aggregate_call_argument_register_boundary.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32! {
    return consume(1, 2, 3, 4, 5, 6, make()?)
}

func make(): Pair! {
    return Pair{ a: 10, b: 20, c: 42, d: 7 }
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, pair: Pair): i32 {
    return pair.c
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
fn run_command_returns_propagated_indirect_aggregate_call_argument_at_register_boundary_exit_code()
{
    let project =
        TempProject::new("cli-run-propagated-indirect-aggregate-call-argument-register-boundary");
    let source = project.write_source(
        "propagated_indirect_aggregate_call_argument_register_boundary.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32! {
    return consume(1, 2, 3, 4, 5, 6, 7, make()?)
}

func make(): Big! {
    return Big{ first: 10, second: 20, code: 42 }
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, value: Big): i32 {
    return value.code
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
fn run_command_returns_propagated_small_direct_aggregate_call_argument_field_exit_code() {
    let project = TempProject::new("cli-run-propagated-small-direct-aggregate-call-argument-field");
    let source = project.write_source(
        "propagated_small_direct_aggregate_call_argument_field.nct",
        r#"struct Code {
    value: i32
}

func main(): i32! {
    return consume(make()?)
}

func make(): Code! {
    return Code{ value: 42 }
}

func consume(code: Code): i32 {
    return code.value
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
fn run_command_returns_propagated_five_byte_direct_aggregate_call_argument_field_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-five-byte-direct-aggregate-call-argument-field");
    let source = project.write_source(
        "propagated_five_byte_direct_aggregate_call_argument_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32! {
    return consume(make()?)
}

func make(): Bytes! {
    return Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 42 }
}

func consume(bytes: Bytes): i32 {
    if bytes.fifth == 42 {
        return 42
    } else {
        return 1
    }
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
fn run_command_returns_shifted_fallible_five_byte_direct_aggregate_argument_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-five-byte-direct-aggregate-call-arg-between-scalars");
    let source = project.write_source(
        "propagated_five_byte_direct_aggregate_call_arg_between_scalars.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32! {
    return consume(5, make()?, 42)
}

func make(): Bytes! {
    return Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 41 }
}

func consume(prefix: i32, bytes: Bytes, suffix: i32): i32 {
    if bytes.fifth == 41 {
        return suffix
    } else {
        return 1
    }
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
fn run_command_returns_caught_direct_aggregate_call_argument_field_exit_code() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-argument-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_argument_field.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    return consume(make() catch error {
        return Error.new("app.main", error.message)
    })
}

func make(): Pair! {
    return Pair{ first: 7, second: 42 }
}

func consume(pair: Pair): i32 {
    return pair.second
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
fn run_command_reports_caught_direct_aggregate_call_argument_failure() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-argument-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_argument_failure.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    return consume(make() catch error {
        return Error.new("app.main", error.message)
    })
}

func make(): Pair! {
    return Error.new("app.make", "failed")
}

func consume(pair: Pair): i32 {
    return pair.second
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.main: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_caught_indirect_aggregate_call_argument_field_exit_code() {
    let project = TempProject::new("cli-run-caught-indirect-aggregate-call-argument-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_argument_field.nct",
        r#"from std/error import Error

struct Big {
    first: usize
    second: usize
    third: usize
    code: i32
}

func main(): i32! {
    return consume(make() catch error {
        return Error.new("app.main", error.message)
    })
}

func make(): Big! {
    return Big{ first: 1, second: 2, third: 3, code: 42 }
}

func consume(value: Big): i32 {
    return value.code
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
fn run_command_returns_caught_direct_aggregate_call_return_field_exit_code() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-return-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_return_field.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    var pair = forward()?
    return pair.second
}

func forward(): Pair! {
    return make() catch error {
        return Error.new("app.forward", error.message)
    }
}

func make(): Pair! {
    return Pair{ first: 7, second: 42 }
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
fn run_command_returns_caught_direct_aggregate_call_comparison_field_exit_code() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-comparison-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_comparison_field.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    if (make() catch error {
        return Error.new("app.main", error.message)
    }).second == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Pair! {
    return Pair{ first: 7, second: 42 }
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
fn run_command_reports_caught_direct_aggregate_call_comparison_failure() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-comparison-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_comparison_failure.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    if (make() catch error {
        return Error.new("app.main", error.message)
    }).second == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Pair! {
    return Error.new("app.make", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.main: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reports_caught_direct_aggregate_call_return_failure() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-return-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_return_failure.nct",
        r#"from std/error import Error

struct Pair {
    first: i32
    second: i32
}

func main(): i32! {
    var pair = forward()?
    return pair.second
}

func forward(): Pair! {
    return make() catch error {
        return Error.new("app.forward", error.message)
    }
}

func make(): Pair! {
    return Error.new("app.make", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.forward: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_caught_indirect_aggregate_call_return_field_exit_code() {
    let project = TempProject::new("cli-run-caught-indirect-aggregate-call-return-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_return_field.nct",
        r#"from std/error import Error

struct Big {
    first: usize
    second: usize
    third: usize
    code: i32
}

func main(): i32! {
    var value = forward()?
    return value.code
}

func forward(): Big! {
    return make() catch error {
        return Error.new("app.forward", error.message)
    }
}

func make(): Big! {
    return Big{ first: 1, second: 2, third: 3, code: 42 }
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
fn run_command_returns_caught_indirect_aggregate_call_comparison_field_exit_code() {
    let project = TempProject::new("cli-run-caught-indirect-aggregate-call-comparison-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_comparison_field.nct",
        r#"from std/error import Error

struct Big {
    first: usize
    second: usize
    third: usize
    code: i32
}

func main(): i32! {
    if (make() catch error {
        return Error.new("app.main", error.message)
    }).code == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Big! {
    return Big{ first: 1, second: 2, third: 3, code: 42 }
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
fn run_command_returns_caught_aggregate_member_assignment_field_exit_code() {
    let project = TempProject::new("cli-run-caught-aggregate-member-assignment-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_member_assignment_field.nct",
        r#"from std/error import Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 1, ok: false, code: 2, len: 3 },
        tail: 4,
    }
    packet.header = source() catch error {
        return Error.new("app.main", error.message)
    }
    return packet.header.code
}

func source(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_caught_aggregate_struct_literal_field_exit_code() {
    let project = TempProject::new("cli-run-caught-aggregate-struct-literal-field");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_struct_literal_field.nct",
        r#"from std/error import Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    let packet = Packet{
        prefix: 1,
        header: source() catch error {
            return Error.new("app.main", error.message)
        },
        tail: 2,
    }
    return packet.header.code
}

func source(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_reports_caught_aggregate_struct_literal_field_failure() {
    let project = TempProject::new("cli-run-caught-aggregate-struct-literal-field-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_struct_literal_field_failure.nct",
        r#"from std/error import Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    let packet = Packet{
        prefix: 1,
        header: source() catch error {
            return Error.new("app.main", error.message)
        },
        tail: 2,
    }
    return packet.header.code
}

func source(): Header! {
    return Error.new("app.source", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.main: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reports_caught_indirect_aggregate_call_return_failure() {
    let project = TempProject::new("cli-run-caught-indirect-aggregate-call-return-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_return_failure.nct",
        r#"from std/error import Error

struct Big {
    first: usize
    second: usize
    third: usize
    code: i32
}

func main(): i32! {
    var value = forward()?
    return value.code
}

func forward(): Big! {
    return make() catch error {
        return Error.new("app.forward", error.message)
    }
}

func make(): Big! {
    return Error.new("app.make", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.forward: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_indirect_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-indirect-aggregate-value-arg");
    let source = project.write_source(
        "indirect_aggregate_value_arg.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 99 }
    let len: usize = length(text)
    if len == 42 {
        return 42
    } else {
        return 1
    }
}

func length(text: Text): usize {
    return text.len
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
fn run_command_returns_readwrite_borrowed_aggregate_field_update_exit_code() {
    let project = TempProject::new("cli-run-readwrite-borrowed-aggregate-field-update");
    let source = project.write_source(
        "readwrite_borrowed_aggregate_field_update.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    var header = Header{ tag: 7, ok: true, code: 1, len: 11 }
    set_code(&+header)
    return header.code
}

func set_code(header: &+Header): void {
    header.code = 42
    return
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
fn run_command_returns_nested_borrowed_aggregate_field_exit_code() {
    let project = TempProject::new("cli-run-nested-borrowed-aggregate-field");
    let source = project.write_source(
        "nested_borrowed_aggregate_field.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    let result = read_code(&packet)
    return result
}

func read_code(packet: &Packet): i32 {
    return packet.header.code
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
fn run_command_returns_nested_aggregate_struct_literal_call_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-struct-literal-call-field");
    let source = project.write_source(
        "nested_aggregate_struct_literal_call_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: make_header(),
        tail: 99,
    }
    return packet.header.code
}

func make_header(): Header {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_nested_aggregate_struct_literal_fallible_call_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-struct-literal-fallible-call-field");
    let source = project.write_source(
        "nested_aggregate_struct_literal_fallible_call_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    let packet = Packet{
        prefix: 1,
        header: make_header()?,
        tail: 99,
    }
    return packet.header.code
}

func make_header(): Header! {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_nested_aggregate_struct_literal_call_member_field_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-struct-literal-call-member-field");
    let source = project.write_source(
        "nested_aggregate_struct_literal_call_member_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: make().header,
        tail: 99,
    }
    return packet.header.code
}

func make(): Packet {
    return Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 2,
    }
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
fn run_command_returns_nested_aggregate_struct_literal_fallible_call_member_field_exit_code() {
    let project =
        TempProject::new("cli-run-nested-aggregate-struct-literal-fallible-call-member-field");
    let source = project.write_source(
        "nested_aggregate_struct_literal_fallible_call_member_field.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    let packet = Packet{
        prefix: 1,
        header: make()?.header,
        tail: 99,
    }
    return packet.header.code
}

func make(): Packet! {
    return Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 2,
    }
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
fn run_command_returns_tail_position_borrowed_aggregate_call_exit_code() {
    let project = TempProject::new("cli-run-tail-position-borrowed-aggregate-call");
    let source = project.write_source(
        "tail_position_borrowed_aggregate_call.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    return read_code(&packet)
}

func read_code(packet: &Packet): i32 {
    return packet.header.code
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
fn run_command_returns_nested_aggregate_field_assignment_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-field-assignment");
    let source = project.write_source(
        "nested_aggregate_field_assignment.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: true, code: 1, len: 11 },
        tail: 99,
    }
    packet.header.code = 42
    return packet.header.code
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
fn run_command_returns_nested_aggregate_field_copy_assignment_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-field-copy-assignment");
    let source = project.write_source(
        "nested_aggregate_field_copy_assignment.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    let header = Header{ tag: 8, ok: true, code: 42, len: 12 }
    packet.header = header
    return packet.header.code
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
fn run_command_returns_borrowed_nested_aggregate_field_copy_assignment_exit_code() {
    let project = TempProject::new("cli-run-borrowed-nested-aggregate-field-copy-assignment");
    let source = project.write_source(
        "borrowed_nested_aggregate_field_copy_assignment.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    let header = Header{ tag: 8, ok: true, code: 42, len: 12 }
    set_header(&+packet, header)
    return packet.header.code
}

func set_header(packet: &+Packet, header: Header): void {
    packet.header = header
    return
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
fn run_command_returns_nested_aggregate_field_call_assignment_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-field-call-assignment");
    let source = project.write_source(
        "nested_aggregate_field_call_assignment.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make_header()
    return packet.header.code
}

func make_header(): Header {
    return Header{ tag: 8, ok: true, code: 42, len: 12 }
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
fn run_command_returns_nested_aggregate_field_member_assignment_from_call_result_exit_code() {
    let project = TempProject::new("cli-run-nested-aggregate-field-member-assignment-call-result");
    let source = project.write_source(
        "nested_aggregate_field_member_assignment_call_result.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make().header
    return packet.header.code
}

func make(): Packet {
    return Packet{
        prefix: 1,
        header: Header{ tag: 8, ok: true, code: 42, len: 12 },
        tail: 2,
    }
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
fn run_command_returns_nested_aggregate_field_member_assignment_from_fallible_call_result_exit_code()
 {
    let project =
        TempProject::new("cli-run-nested-aggregate-field-member-assignment-fallible-call-result");
    let source = project.write_source(
        "nested_aggregate_field_member_assignment_fallible_call_result.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32! {
    var packet = Packet{
        prefix: 1,
        header: Header{ tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make()?.header
    return packet.header.code
}

func make(): Packet! {
    return Packet{
        prefix: 1,
        header: Header{ tag: 8, ok: true, code: 42, len: 12 },
        tail: 2,
    }
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
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.failed: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_reports_fallible_entry_failure_multi_line_message() {
    let project = TempProject::new("cli-run-fallible-failure-multi-line");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", """
        failed
        later
        """)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.failed: failed\nlater\n");
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
        stderr.contains("`return` value has type `&str`, but function `main` returns `i32`"),
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

#[test]
fn check_command_accepts_target_option() {
    let project = TempProject::new("cli-check-target");
    let source = project.write_source(
        "target.nct",
        r#"func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter(
        &project,
        [
            "check",
            source.to_str().unwrap(),
            "--target",
            "arm64-darwin",
        ],
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

    fn write_nocter_home_file(&self, relative: &str, text: &str) {
        let path = self.nocter_home().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
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
