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
        r#"use std/math.answer

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
        r#"use std/math.answer as imported_answer

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
fn run_command_returns_alias_parameter_and_return_exit_code() {
    let project = TempProject::new("cli-run-alias-parameter-return");
    let source = project.write_source(
        "alias_parameter_return.nct",
        r#"type Exit = i32

func main(): i32 {
    return answer(42)
}

func answer(value: Exit): Exit {
    return value
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
fn run_command_accepts_alias_entry_return_type() {
    let project = TempProject::new("cli-run-alias-entry-return");
    let source = project.write_source(
        "alias_entry_return.nct",
        r#"type Exit = i32

func main(): Exit {
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
fn run_command_returns_usize_entry_exit_code() {
    let project = TempProject::new("cli-run-usize-entry-return");
    let source = project.write_source(
        "usize_entry_return.nct",
        r#"func main(): usize {
    return 23
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(23),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_alias_view_signature_for_call_arguments() {
    let project = TempProject::new("cli-run-alias-view-signature-call");
    let source = project.write_source(
        "alias_view_signature_call.nct",
        r#"type Exit = i32
type Text = str

func main(): i32 {
    return length("Nocter")
}

func length(text: &Text): Exit {
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
fn run_command_returns_aggregate_alias_parameter_and_return_exit_code() {
    let project = TempProject::new("cli-run-aggregate-alias-parameter-return");
    let source = project.write_source(
        "aggregate_alias_parameter_return.nct",
        r#"copy struct Pair {
    left: i32
    right: i32
}

type PairAlias = Pair

func main(): i32 {
    let pair = make()
    return sum(pair)
}

func make(): PairAlias {
    return PairAlias{ left: 20, right: 22 }
}

func sum(pair: PairAlias): i32 {
    return pair.left + pair.right
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
        r#"use std/flags.ready

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
        r#"use std/math.add_one
use std/math.base

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
fn run_command_returns_imported_direct_aggregate_call_exit_code() {
    let project = TempProject::new("cli-run-imported-direct-aggregate-call");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Pair {
    pub first: i32
    pub second: i32
}

pub func make_pair(): Pair {
    return Pair{ first: 7, second: 42 }
}

pub func read_second(pair: Pair): i32 {
    return pair.second
}
"#,
    );
    let source = project.write_source(
        "imported_direct_aggregate_call.nct",
        r#"use std/text.{Pair, make_pair, read_second}

func main(): i32 {
    let pair = make_pair()
    return read_second(pair)
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
fn run_command_returns_imported_indirect_aggregate_call_exit_code() {
    let project = TempProject::new("cli-run-imported-indirect-aggregate-call");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Big {
    pub first: usize
    pub second: usize
    pub code: i32
}

pub func make_big(): Big {
    return Big{ first: 1, second: 2, code: 42 }
}

pub func read_code(value: Big): i32 {
    return value.code
}
"#,
    );
    let source = project.write_source(
        "imported_indirect_aggregate_call.nct",
        r#"use std/text.{Big, make_big, read_code}

func main(): i32 {
    let value = make_big()
    return read_code(value)
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
fn run_command_returns_imported_stack_passed_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-imported-stack-passed-direct-aggregate-arg");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Bytes {
    pub first: u8
    pub second: u8
    pub third: u8
    pub fourth: u8
    pub fifth: u8
    pub sixth: u8
    pub seventh: u8
    pub eighth: u8
    pub ninth: u8
}

pub func read_ninth(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, bytes: Bytes): i32 {
    if bytes.ninth == 42 {
        return 42
    } else {
        return 1
    }
}
"#,
    );
    let source = project.write_source(
        "imported_stack_passed_direct_aggregate_arg.nct",
        r#"use std/text.{Bytes, read_ninth}

func main(): i32 {
    return read_ninth(1, 2, 3, 4, 5, 6, 7, 8, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
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
fn run_command_returns_imported_stack_passed_indirect_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-imported-stack-passed-indirect-aggregate-arg");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Big {
    pub first: usize
    pub second: usize
    pub code: i32
}

pub func read_code(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, value: Big): i32 {
    return value.code
}
"#,
    );
    let source = project.write_source(
        "imported_stack_passed_indirect_aggregate_arg.nct",
        r#"use std/text.{Big, read_code}

func main(): i32 {
    return read_code(1, 2, 3, 4, 5, 6, 7, 8, Big{ first: 10, second: 20, code: 42 })
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
fn run_command_returns_u8_normal_and_tail_calls_exit_code() {
    let project = TempProject::new("cli-run-u8-normal-tail-calls");
    let source = project.write_source(
        "u8_normal_tail_calls.nct",
        r#"func main(): i32 {
    let byte: u8 = forward(42)
    return byte as i32
}

func forward(byte: u8): u8 {
    return identity(byte)
}

func identity(byte: u8): u8 {
    return byte
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
fn run_command_returns_scalar_compound_assignment_exit_code() {
    let project = TempProject::new("cli-run-scalar-compound-assignment");
    let source = project.write_source(
        "scalar_compound_assignment.nct",
        r#"func main(): i32 {
    var count = 40
    count += one()
    count *= 2
    count -= 40
    count /= 2
    var size: usize = 47
    size %= 5
    if count == 21 && size == 2 {
        return 23
    } else {
        return 1
    }
}

func one(): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(23),
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
fn run_command_writes_reassigned_str_local() {
    let project = TempProject::new("cli-run-str-var-assignment");
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    let source = project.write_source(
        "str_var_assignment.nct",
        r#"use std/io.write

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
        r#"use std/sizes.size

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
fn run_command_returns_usize_range_for_exit_code() {
    let project = TempProject::new("cli-run-usize-range-for");
    let source = project.write_source(
        "usize_range_for.nct",
        r#"func main(): usize {
    let limit: usize = 4
    var total: usize = 0
    for value in 0..<limit {
        total = total + value
    }
    return total
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(6),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_binding_reusing_range_for_name_after_loop() {
    let project = TempProject::new("cli-run-range-for-name-reuse");
    let source = project.write_source(
        "range_for_name_reuse.nct",
        r#"func main(): i32 {
    for value in 0..<2 {
    }
    let value = 5
    return value
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
fn run_command_traps_stack_passed_never_call() {
    let project = TempProject::new("cli-run-stack-passed-never-call");
    let source = project.write_source(
        "stack_passed_never_call.nct",
        r#"use std/process.abort

func main(): i32 {
    return fail(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func fail(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, i: i32): never {
    abort()
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
fn run_command_traps_aggregate_argument_never_call() {
    let project = TempProject::new("cli-run-aggregate-argument-never-call");
    let source = project.write_source(
        "aggregate_argument_never_call.nct",
        r#"use std/process.abort

copy struct Big {
    first: usize
    second: usize
    code: usize
}

func main(): i32 {
    let value = Big{ first: 1, second: 2, code: 42 }
    return fail(value)
}

func fail(value: Big): never {
    abort()
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
fn run_command_preserves_direct_aggregate_parameter_after_normal_call() {
    let project = TempProject::new("cli-run-preserve-direct-aggregate-param");
    let source = project.write_source(
        "preserve_direct_aggregate_param.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return consume(Header{ tag: 7, ok: true, code: 42, len: 11 })
}

func consume(header: Header): i32 {
    let ignored = noise()
    return header.code
}

func noise(): i32 {
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
fn run_command_preserves_direct_aggregate_first_field_after_normal_call() {
    let project = TempProject::new("cli-run-preserve-direct-aggregate-first-field");
    let source = project.write_source(
        "preserve_direct_aggregate_first_field.nct",
        r#"struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    return consume(Pair{ first: 42, second: 7 })
}

func consume(pair: Pair): i32 {
    let ignored = noise()
    return pair.first
}

func noise(): i32 {
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
fn run_command_preserves_indirect_aggregate_parameter_after_normal_call() {
    let project = TempProject::new("cli-run-preserve-indirect-aggregate-param");
    let source = project.write_source(
        "preserve_indirect_aggregate_param.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32 {
    return consume(Big{ first: 10, second: 20, code: 42 })
}

func consume(value: Big): i32 {
    let ignored = noise()
    return value.code
}

func noise(): i32 {
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
fn run_command_runs_direct_aggregate_return_scope_drops() {
    let project = TempProject::new("cli-run-direct-aggregate-return-scope-drops");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "direct_aggregate_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

copy struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose()
    return pair.second
}

func choose(): Pair {
    var file = File{ fd: 3 }
    return Pair{ first: 1, second: 42 }
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
    assert_eq!(output.stdout, b"drop\n");
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
fn run_command_returns_nine_byte_direct_aggregate_value_argument_field_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-direct-aggregate-value-arg");
    let source = project.write_source(
        "nine_byte_direct_aggregate_value_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    return consume(Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
}

func consume(bytes: Bytes): i32 {
    if bytes.ninth == 42 {
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
fn run_command_returns_shifted_nine_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-direct-aggregate-arg-between-scalars");
    let source = project.write_source(
        "nine_byte_direct_aggregate_arg_between_scalars.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    return consume(5, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 41,
    }, 42)
}

func consume(prefix: i32, bytes: Bytes, suffix: i32): i32 {
    if bytes.ninth == 41 {
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
fn run_command_returns_boundary_nine_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-direct-aggregate-arg-register-boundary");
    let source = project.write_source(
        "nine_byte_direct_aggregate_arg_register_boundary.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, bytes: Bytes): i32 {
    if bytes.ninth == 42 {
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
fn run_command_returns_nine_byte_direct_aggregate_call_result_field_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-direct-aggregate-call-result-field");
    let source = project.write_source(
        "nine_byte_direct_aggregate_call_result_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    if make().ninth == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes {
    return Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
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
fn run_command_returns_propagated_nine_byte_direct_aggregate_call_return_field_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-nine-byte-direct-aggregate-call-return-field");
    let source = project.write_source(
        "propagated_nine_byte_direct_aggregate_call_return_field.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32! {
    if make()?.ninth == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes! {
    return Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
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
fn run_command_returns_shifted_fallible_nine_byte_direct_aggregate_argument_exit_code() {
    let project =
        TempProject::new("cli-run-propagated-nine-byte-direct-aggregate-call-arg-between-scalars");
    let source = project.write_source(
        "propagated_nine_byte_direct_aggregate_call_arg_between_scalars.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32! {
    return consume(5, make()?, 42)
}

func make(): Bytes! {
    return Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 41,
    }
}

func consume(prefix: i32, bytes: Bytes, suffix: i32): i32 {
    if bytes.ninth == 41 {
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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_argument_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_argument_failure.nct",
        r#"use std/error.Error

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
fn run_command_preserves_propagated_failure_payload_after_scope_drop() {
    let project = TempProject::new("cli-run-propagate-cleanup-drop");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "propagate_cleanup_drop.nct",
        r#"use std/error.Error

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        touch2(file.fd, 99)
        return
    }
}

func main(): void! {
    var file = File{ fd: 3 }
    fail()?
}

func fail(): void! {
    return Error.new("app.failed", "failed")
}

func touch2(a: i32, b: i32): void {
    return
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
fn run_command_runs_catch_failure_scope_drop_cleanup() {
    let project = TempProject::new("cli-run-catch-cleanup-drop");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "catch_cleanup_drop.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File{ fd: 3 }
    let value = fail() catch error {
        return Error.new("app.outer", error.message)
    }
    return value
}

func fail(): i32! {
    return Error.new("app.inner", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n");
    assert_eq!(output.stderr, b"app.outer: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_replacement_and_scope_end_drops() {
    let project = TempProject::new("cli-run-replacement-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "replacement_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File{ fd: 1 }
    file = File{ fd: 42 }
    return file.fd
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
    assert_eq!(output.stdout, b"drop\ndrop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_reinitialization_after_explicit_drop() {
    let project = TempProject::new("cli-run-reinitialize-after-explicit-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "reinitialize_after_explicit_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File{ fd: 1 }
    drop file
    file = File{ fd: 42 }
    return file.fd
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
    assert_eq!(output.stdout, b"drop\ndrop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_preserves_terminal_if_return_value_after_scope_drop() {
    let project = TempProject::new("cli-run-terminal-if-return-drop");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "terminal_if_return_drop.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File{ fd: 42 }
    if true {
        return file.fd
    } else {
        return 7
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
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nonterminal_outer_scalar_assignment_exit_code() {
    let project = TempProject::new("cli-run-nonterminal-outer-scalar-assignment");
    let source = project.write_source(
        "nonterminal_outer_scalar_assignment.nct",
        r#"func main(): i32 {
    var value = 1
    if true {
        value = 2
    }
    while false {
        value = 3
    }
    return value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_outer_explicit_drop_before_nonterminal_if_return_once() {
    let project = TempProject::new("cli-run-nonterminal-if-outer-drop-return");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "nonterminal_if_outer_drop_return.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File{ fd: 7 }
    if true {
        drop file
        return 7
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_argument_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_return_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_comparison_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_comparison_failure.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_direct_aggregate_call_return_failure.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_return_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_comparison_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_member_assignment_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_struct_literal_field.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_aggregate_struct_literal_field_failure.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "caught_indirect_aggregate_call_return_failure.nct",
        r#"use std/error.Error

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
    let len: usize = length(move text)
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
fn run_command_returns_stack_passed_scalar_argument_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-scalar-arg");
    let source = project.write_source(
        "stack_passed_scalar_arg.nct",
        r#"func main(): i32 {
    return ninth(1, 2, 3, 4, 5, 6, 7, 8, 42)
}

func ninth(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, i: i32): i32 {
    return i
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
fn run_command_returns_stack_passed_str_argument_len_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-str-arg-len");
    let source = project.write_source(
        "stack_passed_str_arg_len.nct",
        r#"func main(): i32 {
    let len: usize = length(1, 2, 3, 4, 5, 6, 7, 8, "Nocter")
    if len == 6 {
        return 42
    } else {
        return 1
    }
}

func length(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, text: &str): usize {
    return text.len()
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
fn run_command_returns_split_register_stack_passed_str_argument_first_byte_exit_code() {
    let project = TempProject::new("cli-run-split-register-stack-str-arg-first-byte");
    let source = project.write_source(
        "split_register_stack_str_arg_first_byte.nct",
        r#"func main(): i32 {
    let value: i32 = first_byte(1, 2, 3, 4, 5, 6, 7, "Nocter")
    if value == 78 {
        return 42
    } else {
        return 1
    }
}

func first_byte(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, text: &str): i32 {
    return text[0] as i32
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
fn run_command_returns_loop_break_exit_code() {
    let project = TempProject::new("cli-run-loop-break");
    let source = project.write_source(
        "loop_break.nct",
        r#"func main(): i32 {
    var value = 0
    loop {
        value = 42
        break
    }
    return value
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
fn run_command_returns_terminal_loop_exit_code() {
    let project = TempProject::new("cli-run-terminal-loop");
    let source = project.write_source(
        "terminal_loop.nct",
        r#"func main(): i32 {
    loop {
        return 42
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
fn run_command_returns_alias_i32_conversion_exit_code() {
    let project = TempProject::new("cli-run-alias-i32-conversion");
    let source = project.write_source(
        "alias_i32_conversion.nct",
        r#"type Exit = i32

func main(): i32 {
    return "A"[0] as Exit
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(65),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_forwarded_stack_passed_str_argument_first_byte_exit_code() {
    let project = TempProject::new("cli-run-forwarded-stack-passed-str-arg-first-byte");
    let source = project.write_source(
        "forwarded_stack_passed_str_arg_first_byte.nct",
        r#"func main(): i32 {
    let value: i32 = forward(1, 2, 3, 4, 5, 6, 7, 8, "Nocter")
    if value == 78 {
        return 42
    } else {
        return 1
    }
}

func forward(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, text: &str): i32 {
    return first_byte(1, 2, 3, 4, 5, 6, 7, 8, text)
}

func first_byte(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, text: &str): i32 {
    return text[0] as i32
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
fn run_command_returns_stack_passed_indirect_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-indirect-aggregate-arg");
    let source = project.write_source(
        "stack_passed_indirect_aggregate_arg.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 99 }
    let len: usize = length(1, 2, 3, 4, 5, 6, 7, 8, move text)
    if len == 42 {
        return 42
    } else {
        return 1
    }
}

func length(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, text: Text): usize {
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
fn run_command_returns_split_stack_passed_direct_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-split-stack-direct-aggregate-arg");
    let source = project.write_source(
        "split_stack_direct_aggregate_arg.nct",
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    return check(1, 2, 3, 4, 5, 6, 7, pair)
}

func check(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
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
fn run_command_returns_fully_stack_passed_direct_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-direct-aggregate-arg");
    let source = project.write_source(
        "fully_stack_direct_aggregate_arg.nct",
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair{ a: 10, b: 20, c: 7, d: 5 }
    return check(1, 2, 3, 4, 5, 6, 7, 8, pair)
}

func check(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
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
fn run_command_returns_split_stack_passed_nine_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-split-stack-passed-nine-byte-direct-aggregate-arg");
    let source = project.write_source(
        "split_stack_passed_nine_byte_direct_aggregate_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, bytes: Bytes): i32 {
    if bytes.ninth == 42 {
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
fn run_command_returns_fully_stack_passed_five_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-passed-five-byte-direct-aggregate-arg");
    let source = project.write_source(
        "fully_stack_passed_five_byte_direct_aggregate_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, 8, Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, bytes: Bytes): i32 {
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
fn run_command_returns_fully_stack_passed_nine_byte_direct_aggregate_argument_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-passed-nine-byte-direct-aggregate-arg");
    let source = project.write_source(
        "fully_stack_passed_nine_byte_direct_aggregate_arg.nct",
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, 8, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, bytes: Bytes): i32 {
    if bytes.ninth == 42 {
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
fn run_command_returns_stack_passed_direct_aggregate_parameter_return_by_name_exit_code() {
    let project =
        TempProject::new("cli-run-stack-passed-direct-aggregate-parameter-return-by-name");
    let source = project.write_source(
        "stack_passed_direct_aggregate_parameter_return_by_name.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    let bytes = identity(1, 2, 3, 4, 5, 6, 7, 8, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
    if bytes.ninth == 42 {
        return 42
    } else {
        return 1
    }
}

func identity(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, bytes: Bytes): Bytes {
    return bytes
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
fn run_command_preserves_stack_passed_direct_aggregate_parameter_return_after_normal_call() {
    let project = TempProject::new("cli-run-preserve-stack-direct-aggregate-return-after-call");
    let source = project.write_source(
        "preserve_stack_direct_aggregate_return_after_call.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    let bytes = identity(1, 2, 3, 4, 5, 6, 7, 8, Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    })
    if bytes.ninth == 42 {
        return 42
    } else {
        return 1
    }
}

func identity(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, bytes: Bytes): Bytes {
    let ignored = noise()
    return bytes
}

func noise(): i32 {
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
fn run_command_returns_stack_passed_indirect_aggregate_parameter_return_by_name_exit_code() {
    let project =
        TempProject::new("cli-run-stack-passed-indirect-aggregate-parameter-return-by-name");
    let source = project.write_source(
        "stack_passed_indirect_aggregate_parameter_return_by_name.nct",
        r#"copy struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32 {
    let value = identity(1, 2, 3, 4, 5, 6, 7, 8, Big{ first: 10, second: 20, code: 42 })
    return value.code
}

func identity(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, value: Big): Big {
    return value
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
fn run_command_returns_stack_passed_propagated_direct_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-propagated-direct-aggregate-arg");
    let source = project.write_source(
        "stack_passed_propagated_direct_aggregate_arg.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32! {
    return check(1, 2, 3, 4, 5, 6, 7, make()?)
}

func make(): Pair! {
    return Pair{ a: 10, b: 20, c: 7, d: 5 }
}

func check(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
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
fn run_command_returns_fully_stack_passed_propagated_direct_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-propagated-direct-aggregate-arg");
    let source = project.write_source(
        "fully_stack_propagated_direct_aggregate_arg.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32! {
    return check(1, 2, 3, 4, 5, 6, 7, 8, make()?)
}

func make(): Pair! {
    return Pair{ a: 10, b: 20, c: 7, d: 5 }
}

func check(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
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
fn run_command_returns_stack_passed_propagated_indirect_aggregate_argument_field_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-propagated-indirect-aggregate-arg");
    let source = project.write_source(
        "stack_passed_propagated_indirect_aggregate_arg.nct",
        r#"struct Big {
    first: usize
    second: usize
    code: i32
}

func main(): i32! {
    return check(1, 2, 3, 4, 5, 6, 7, 8, make()?)
}

func make(): Big! {
    return Big{ first: 10, second: 20, code: 42 }
}

func check(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, value: Big): i32 {
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
fn run_command_passes_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-run-aggregate-field-borrow-argument");
    let source = project.write_source(
        "aggregate_field_borrow_argument.nct",
        r#"type IntRef = &i32

copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair{ value: 1 }
    return choose(&pair.value, 42)
}

func choose(value: IntRef, code: i32): i32 {
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
fn run_command_passes_borrowed_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-run-borrowed-aggregate-field-borrow-argument");
    let source = project.write_source(
        "borrowed_aggregate_field_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair{ value: 1 }
    return caller(&pair)
}

func caller(pair: &Pair): i32 {
    return choose(&pair.value, 42)
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
fn run_command_passes_readwrite_borrowed_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-run-readwrite-borrowed-aggregate-field-borrow-argument");
    let source = project.write_source(
        "readwrite_borrowed_aggregate_field_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    var pair = Pair{ value: 1 }
    return caller(&+pair)
}

func caller(pair: &+Pair): i32 {
    return choose(&+pair.value, 42)
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
fn run_command_passes_scalar_parameter_borrow_argument() {
    let project = TempProject::new("cli-run-scalar-parameter-borrow-argument");
    let source = project.write_source(
        "scalar_parameter_borrow_argument.nct",
        r#"func main(): i32 {
    return caller(7)
}

func caller(value: i32): i32 {
    return choose(&value, 42)
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
fn run_command_passes_stack_scalar_parameter_borrow_argument() {
    let project = TempProject::new("cli-run-stack-scalar-parameter-borrow-argument");
    let source = project.write_source(
        "stack_scalar_parameter_borrow_argument.nct",
        r#"func main(): i32 {
    return caller(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func caller(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, value: i32): i32 {
    return choose(&value, 42)
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_stack_passed_borrowed_aggregate_field_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-borrowed-aggregate-field");
    let source = project.write_source(
        "stack_passed_borrowed_aggregate_field.nct",
        r#"struct Packet {
    code: i32
    len: usize
    cap: usize
}

func main(): i32 {
    let packet = Packet{ code: 42, len: 7, cap: 9 }
    return read_code(1, 2, 3, 4, 5, 6, 7, 8, &packet)
}

func read_code(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, packet: &Packet): i32 {
    return packet.code
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
fn run_command_returns_stack_passed_readwrite_borrowed_aggregate_field_update_exit_code() {
    let project =
        TempProject::new("cli-run-stack-passed-readwrite-borrowed-aggregate-field-update");
    let source = project.write_source(
        "stack_passed_readwrite_borrowed_aggregate_field_update.nct",
        r#"struct Packet {
    code: i32
    len: usize
    cap: usize
}

func main(): i32 {
    var packet = Packet{ code: 1, len: 7, cap: 9 }
    set_code(1, 2, 3, 4, 5, 6, 7, 8, &+packet)
    return packet.code
}

func set_code(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, packet: &+Packet): void {
    packet.code = 42
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
fn run_command_preserves_stack_passed_nested_aggregate_field_update_after_normal_call() {
    let project = TempProject::new("cli-run-stack-passed-nested-aggregate-field-update-after-call");
    let source = project.write_source(
        "stack_passed_nested_aggregate_field_update_after_call.nct",
        r#"copy struct Header {
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
        header: Header{ tag: 1, ok: false, code: 1, len: 2 },
        tail: 3,
    }
    set_header(1, 2, 3, 4, 5, 6, 7, 8, &+packet)
    return packet.header.code
}

func set_header(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, packet: &+Packet): void {
    let ignored = noise()
    packet.header = Header{ tag: 7, ok: true, code: 42, len: 11 }
    return
}

func noise(): i32 {
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
fn run_command_returns_non_copy_aggregate_field_replacement_assignment_exit_code() {
    let project = TempProject::new("cli-run-non-copy-aggregate-field-replacement-assignment");
    let source = project.write_source(
        "non_copy_aggregate_field_replacement_assignment.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder{ tag: 1, file: File{ fd: 7 } }
    holder.file = File{ fd: 41 }
    return holder.file.fd + holder.tag
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
fn run_command_returns_borrowed_non_copy_aggregate_field_replacement_assignment_exit_code() {
    let project = TempProject::new("cli-run-borrowed-non-copy-aggregate-field-replacement");
    let source = project.write_source(
        "borrowed_non_copy_aggregate_field_replacement.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder{ tag: 1, file: File{ fd: 7 } }
    replace(&+holder)
    return holder.file.fd + holder.tag
}

func replace(holder: &+Holder): void {
    holder.file = File{ fd: 41 }
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
fn run_command_returns_non_copy_aggregate_field_call_replacement_assignment_exit_code() {
    let project = TempProject::new("cli-run-non-copy-aggregate-field-call-replacement");
    let source = project.write_source(
        "non_copy_aggregate_field_call_replacement.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder{ tag: 1, file: File{ fd: 7 } }
    holder.file = make_file()
    return holder.file.fd + holder.tag
}

func make_file(): File {
    return File{ fd: 41 }
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
fn run_command_returns_outer_aggregate_replacement_inside_while_exit_code() {
    let project = TempProject::new("cli-run-outer-aggregate-replacement-inside-while");
    let source = project.write_source(
        "outer_aggregate_replacement_inside_while.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    var file = File{ fd: 1 }
    var count = 0
    while count == 0 {
        file = File{ fd: 41 }
        count = 1
    }
    return file.fd + count
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
fn run_command_returns_five_byte_copy_aggregate_assignment_exit_code() {
    let project = TempProject::new("cli-run-five-byte-copy-aggregate-assignment");
    let source = project.write_source(
        "five_byte_copy_aggregate_assignment.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    var bytes = Bytes{ first: 1, second: 2, third: 3, fourth: 4, fifth: 1 }
    let replacement = Bytes{ first: 5, second: 6, third: 7, fourth: 8, fifth: 42 }
    bytes = replacement
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
fn run_command_returns_nine_byte_copy_aggregate_assignment_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-copy-aggregate-assignment");
    let source = project.write_source(
        "nine_byte_copy_aggregate_assignment.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    var bytes = Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 1,
    }
    let replacement = Bytes{
        first: 9,
        second: 10,
        third: 11,
        fourth: 12,
        fifth: 13,
        sixth: 14,
        seventh: 15,
        eighth: 16,
        ninth: 42,
    }
    bytes = replacement
    if bytes.ninth == 42 {
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
fn run_command_returns_nine_byte_copy_aggregate_return_by_name_exit_code() {
    let project = TempProject::new("cli-run-nine-byte-copy-aggregate-return-by-name");
    let source = project.write_source(
        "nine_byte_copy_aggregate_return_by_name.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
    sixth: u8
    seventh: u8
    eighth: u8
    ninth: u8
}

func main(): i32 {
    let bytes = make()
    if bytes.ninth == 42 {
        return 42
    } else {
        return 1
    }
}

func make(): Bytes {
    let bytes = Bytes{
        first: 1,
        second: 2,
        third: 3,
        fourth: 4,
        fifth: 5,
        sixth: 6,
        seventh: 7,
        eighth: 8,
        ninth: 42,
    }
    return bytes
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
fn run_command_returns_compound_bool_equality_exit_code() {
    let project = TempProject::new("cli-run-compound-bool-equality");
    let source = project.write_source(
        "compound_bool_equality.nct",
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let unary_same = !ready == blocked
    let logical_same = (ready && !blocked) == ready
    if unary_same && logical_same {
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
fn run_command_returns_compound_bool_equality_nested_call_exit_code() {
    let project = TempProject::new("cli-run-compound-bool-equality-nested-call");
    let source = project.write_source(
        "compound_bool_equality_nested_call.nct",
        r#"func main(): i32 {
    let both = (ready() && other()) == true
    let neither = !(blocked() || closed()) == true
    if both && neither {
        return 31
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}

func blocked(): bool {
    return false
}

func closed(): bool {
    return false
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
fn run_command_returns_fallible_usize_entry_success_exit_code() {
    let project = TempProject::new("cli-run-fallible-usize-success");
    let source = project.write_source(
        "exit_usize_29.nct",
        r#"func main(): usize! {
    return 29
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(29),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_force_unwrap_success_exit_code() {
    let project = TempProject::new("cli-run-optional-force-success");
    let source = project.write_source(
        "optional_force_success.nct",
        r#"func main(): i32 {
    return maybe_answer()!
}

func maybe_answer(): i32? {
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
fn run_command_traps_optional_force_unwrap_none() {
    let project = TempProject::new("cli-run-optional-force-none");
    let source = project.write_source(
        "optional_force_none.nct",
        r#"func main(): i32 {
    return maybe_answer()!
}

func maybe_answer(): i32? {
    return none
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
fn run_command_returns_optional_let_else_success_exit_code() {
    let project = TempProject::new("cli-run-optional-let-else-success");
    let source = project.write_source(
        "optional_let_else_success.nct",
        r#"func main(): i32 {
    let value = maybe_answer() else {
        return 7
    }

    return value
}

func maybe_answer(): i32? {
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
fn run_command_returns_optional_let_else_none_exit_code() {
    let project = TempProject::new("cli-run-optional-let-else-none");
    let source = project.write_source(
        "optional_let_else_none.nct",
        r#"func main(): i32 {
    let value = maybe_answer() else {
        return 7
    }

    return value
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_optional_let_else_never_scope_drop_before_trap() {
    let project = TempProject::new("cli-run-optional-let-else-never-cleanup");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    project.write_nocter_home_file(
        "std/process.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive exit_raw(code: i32): never

pub func exit(code: i32): never {
    exit_raw(code)
}
"#,
    );
    let source = project.write_source(
        "optional_let_else_never_cleanup.nct",
        r#"use std/log.write
use std/process.exit

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32 {
    var file = File{ fd: 3 }
    let value = maybe_answer() else {
        exit(7)
    }

    return value
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"drop\n",
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_default_success_exit_code() {
    let project = TempProject::new("cli-run-optional-default-success");
    let source = project.write_source(
        "optional_default_success.nct",
        r#"func main(): i32 {
    return maybe_answer() ?? 7
}

func maybe_answer(): i32? {
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
fn run_command_returns_optional_default_none_exit_code() {
    let project = TempProject::new("cli-run-optional-default-none");
    let source = project.write_source(
        "optional_default_none.nct",
        r#"func main(): i32 {
    return maybe_answer() ?? 7
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_default_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-default-binding-success");
    let source = project.write_source(
        "optional_default_binding_success.nct",
        r#"func main(): i32 {
    let value = maybe_answer() ?? 7
    return value
}

func maybe_answer(): i32? {
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
fn run_command_returns_optional_default_binding_none_exit_code() {
    let project = TempProject::new("cli-run-optional-default-binding-none");
    let source = project.write_source(
        "optional_default_binding_none.nct",
        r#"func main(): i32 {
    let value = maybe_answer() ?? 7
    return value
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_scalar_default_bindings_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-default-bindings");
    let source = project.write_source(
        "optional_scalar_default_bindings.nct",
        r#"func main(): i32 {
    let byte: u8 = maybe_byte() ?? 1
    let size = maybe_size() ?? 2
    let flag = maybe_flag() ?? false
    let text = maybe_text() ?? "fallback"

    if flag && size == 40 {
        return byte as i32
    } else {
        return 1
    }
}

func maybe_byte(): u8? {
    return 42
}

func maybe_size(): usize? {
    return 40
}

func maybe_flag(): bool? {
    return true
}

func maybe_text(): &str? {
    return "text"
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
fn run_command_returns_optional_scalar_default_binding_fallbacks_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-default-binding-fallbacks");
    let source = project.write_source(
        "optional_scalar_default_binding_fallbacks.nct",
        r#"func main(): i32 {
    let byte: u8 = maybe_byte() ?? 42
    let size = maybe_size() ?? 40
    let flag = maybe_flag() ?? true
    let text = maybe_text() ?? "fallback"

    if flag && size == 40 {
        return byte as i32
    } else {
        return 1
    }
}

func maybe_byte(): u8? {
    return none
}

func maybe_size(): usize? {
    return none
}

func maybe_flag(): bool? {
    return none
}

func maybe_text(): &str? {
    return none
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
fn run_command_returns_optional_scalar_default_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-default-return-success");
    let source = project.write_source(
        "optional_scalar_default_return_success.nct",
        r#"func main(): i32 {
    let byte: u8 = choose_byte()
    let size = choose_size()
    let flag = choose_flag()
    let text = choose_text()

    if flag && size == 40 {
        return byte as i32
    } else {
        return 1
    }
}

func choose_byte(): u8 {
    return maybe_byte() ?? 1
}

func choose_size(): usize {
    return maybe_size() ?? 2
}

func choose_flag(): bool {
    return maybe_flag() ?? false
}

func choose_text(): &str {
    return maybe_text() ?? "fallback"
}

func maybe_byte(): u8? {
    return 42
}

func maybe_size(): usize? {
    return 40
}

func maybe_flag(): bool? {
    return true
}

func maybe_text(): &str? {
    return "text"
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
fn run_command_returns_optional_scalar_default_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-default-return-fallback");
    let source = project.write_source(
        "optional_scalar_default_return_fallback.nct",
        r#"func main(): i32 {
    let byte: u8 = choose_byte()
    let size = choose_size()
    let flag = choose_flag()
    let text = choose_text()

    if flag && size == 40 {
        return byte as i32
    } else {
        return 1
    }
}

func choose_byte(): u8 {
    return maybe_byte() ?? 42
}

func choose_size(): usize {
    return maybe_size() ?? 40
}

func choose_flag(): bool {
    return maybe_flag() ?? true
}

func choose_text(): &str {
    return maybe_text() ?? "fallback"
}

func maybe_byte(): u8? {
    return none
}

func maybe_size(): usize? {
    return none
}

func maybe_flag(): bool? {
    return none
}

func maybe_text(): &str? {
    return none
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
fn run_command_runs_optional_scalar_default_return_scope_drops() {
    let project = TempProject::new("cli-run-optional-scalar-default-return-scope-drops");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "optional_scalar_default_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

func main(): i32 {
    let success = choose_success()
    let fallback = choose_fallback()
    if success == 42 && fallback == 7 {
        return 0
    } else {
        return 1
    }
}

func choose_success(): i32 {
    var file = File{ fd: 3 }
    return maybe_answer_success() ?? 7
}

func choose_fallback(): i32 {
    var file = File{ fd: 4 }
    return maybe_answer_none() ?? 7
}

func maybe_answer_success(): i32? {
    return 42
}

func maybe_answer_none(): i32? {
    return none
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
    assert_eq!(output.stdout, b"drop\ndrop\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_let_else_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-let-else-success");
    let source = project.write_source(
        "optional_direct_aggregate_let_else_success.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() else {
        return 7
    }

    return header.code
}

func maybe_header(): Header? {
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
fn run_command_returns_optional_direct_aggregate_let_else_none_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-let-else-none");
    let source = project.write_source(
        "optional_direct_aggregate_let_else_none.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() else {
        return 7
    }

    return header.code
}

func maybe_header(): Header? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_let_else_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-let-else-success");
    let source = project.write_source(
        "optional_indirect_aggregate_let_else_success.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() else {
        return 7
    }

    if value.second == 42 {
        return 42
    } else {
        return 1
    }
}

func maybe_triple(): Triple? {
    return Triple{ first: 1, second: 42, third: 3 }
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
fn run_command_returns_optional_indirect_aggregate_let_else_none_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-let-else-none");
    let source = project.write_source(
        "optional_indirect_aggregate_let_else_none.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() else {
        return 7
    }

    if value.second == 42 {
        return 42
    } else {
        return 1
    }
}

func maybe_triple(): Triple? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_default_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-default-binding-success");
    let source = project.write_source(
        "optional_direct_aggregate_default_binding_success.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
    return header.code
}

func maybe_header(): Header? {
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
fn run_command_returns_optional_direct_aggregate_default_binding_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-default-binding-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_default_binding_fallback.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
    return header.code
}

func maybe_header(): Header? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_default_binding_copy_fallback_exit_code() {
    let project =
        TempProject::new("cli-run-optional-direct-aggregate-default-binding-copy-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_default_binding_copy_fallback.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let fallback = Header{ tag: 1, ok: false, code: 7, len: 2 }
    let header = maybe_header() ?? fallback
    return header.code
}

func maybe_header(): Header? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_default_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-default-binding-success");
    let source = project.write_source(
        "optional_indirect_aggregate_default_binding_success.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() ?? Triple{ first: 1, second: 7, third: 3 }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func maybe_triple(): Triple? {
    return Triple{ first: 1, second: 42, third: 3 }
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
fn run_command_returns_optional_indirect_aggregate_default_binding_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-default-binding-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_default_binding_fallback.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() ?? Triple{ first: 1, second: 7, third: 3 }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func maybe_triple(): Triple? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_default_binding_call_fallback_exit_code() {
    let project =
        TempProject::new("cli-run-optional-indirect-aggregate-default-binding-call-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_default_binding_call_fallback.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() ?? fallback_triple()
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func maybe_triple(): Triple? {
    return none
}

func fallback_triple(): Triple {
    return Triple{ first: 1, second: 7, third: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_default_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-default-return-success");
    let source = project.write_source(
        "optional_direct_aggregate_default_return_success.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = choose()
    return header.code
}

func choose(): Header {
    return maybe_header() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
}

func maybe_header(): Header? {
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
fn run_command_returns_optional_direct_aggregate_default_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-default-return-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_default_return_fallback.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = choose()
    return header.code
}

func choose(): Header {
    return maybe_header() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
}

func maybe_header(): Header? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_optional_direct_aggregate_default_return_scope_drops() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-default-return-scope-drops");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "optional_direct_aggregate_default_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let success = choose_success()
    let fallback = choose_fallback()
    return success.code + fallback.code
}

func choose_success(): Header {
    var file = File{ fd: 3 }
    return maybe_header_success() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
}

func choose_fallback(): Header {
    var file = File{ fd: 4 }
    return maybe_header_none() ?? Header{ tag: 1, ok: false, code: 7, len: 2 }
}

func maybe_header_success(): Header? {
    return Header{ tag: 7, ok: true, code: 42, len: 11 }
}

func maybe_header_none(): Header? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(49),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\ndrop\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_default_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-default-return-success");
    let source = project.write_source(
        "optional_indirect_aggregate_default_return_success.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = choose()
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func choose(): Triple {
    return maybe_triple() ?? Triple{ first: 1, second: 7, third: 3 }
}

func maybe_triple(): Triple? {
    return Triple{ first: 1, second: 42, third: 3 }
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
fn run_command_returns_optional_indirect_aggregate_default_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-default-return-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_default_return_fallback.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = choose()
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func choose(): Triple {
    return maybe_triple() ?? Triple{ first: 1, second: 7, third: 3 }
}

func maybe_triple(): Triple? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_runs_optional_indirect_aggregate_default_return_scope_drops() {
    let project =
        TempProject::new("cli-run-optional-indirect-aggregate-default-return-scope-drops");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "optional_indirect_aggregate_default_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        write("drop\n")!
        return
    }
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let success = choose_success()
    let fallback = choose_fallback()
    return code(success.second + fallback.second)
}

func code(value: usize): i32 {
    if value == 49 {
        return 49
    } else {
        return 1
    }
}

func choose_success(): Triple {
    var file = File{ fd: 3 }
    return maybe_triple_success() ?? Triple{ first: 1, second: 7, third: 3 }
}

func choose_fallback(): Triple {
    var file = File{ fd: 4 }
    return maybe_triple_none() ?? Triple{ first: 1, second: 7, third: 3 }
}

func maybe_triple_success(): Triple? {
    return Triple{ first: 1, second: 42, third: 3 }
}

func maybe_triple_none(): Triple? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(49),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\ndrop\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_force_unwrap_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-force");
    let source = project.write_source(
        "optional_direct_aggregate_force.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header()!
    return header.code
}

func maybe_header(): Header? {
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
fn run_command_traps_optional_direct_aggregate_force_unwrap_none() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-force-none");
    let source = project.write_source(
        "optional_direct_aggregate_force_none.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header()!
    return header.code
}

func maybe_header(): Header? {
    return none
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
fn run_command_returns_optional_indirect_aggregate_force_unwrap_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-force");
    let source = project.write_source(
        "optional_indirect_aggregate_force.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple()!
    if value.second == 42 {
        return 42
    } else {
        return 1
    }
}

func maybe_triple(): Triple? {
    return Triple{ first: 1, second: 42, third: 3 }
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
fn run_command_traps_optional_indirect_aggregate_force_unwrap_none() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-force-none");
    let source = project.write_source(
        "optional_indirect_aggregate_force_none.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple()!
    if value.second == 42 {
        return 42
    } else {
        return 1
    }
}

func maybe_triple(): Triple? {
    return none
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
fn run_command_reports_fallible_entry_failure() {
    let project = TempProject::new("cli-run-fallible-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"use std/error.Error

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

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail.nct",
        r#"use std/error.Error

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
    assert!(
        stderr.contains("2 |     return \"bad\""),
        "expected source line, got:\n{stderr}"
    );
    assert!(
        stderr.contains("  |            ^^^^^"),
        "expected source underline, got:\n{stderr}"
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
