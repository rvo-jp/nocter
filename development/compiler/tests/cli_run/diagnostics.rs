use super::*;

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
fn run_command_traps_invalid_nonlegacy_integer_operations() {
    let cases = [
        (
            "i8-overflow",
            r#"func value(): i8 {
    return maximum() + one()
}

func maximum(): i8 { return 127 }
func one(): i8 { return 1 }

func main(): i32 {
    if value() == 0 { return 0 }
    return 1
}
"#,
        ),
        (
            "i64-division-overflow",
            r#"func value(): i64 {
    return minimum() / minus_one()
}

func minimum(): i64 { return -9223372036854775808 }
func minus_one(): i64 { return -1 }

func main(): i32 {
    if value() == 0 { return 0 }
    return 1
}
"#,
        ),
        (
            "u64-division-zero",
            r#"func value(): u64 {
    return one() / zero()
}

func one(): u64 { return 1 }
func zero(): u64 { return 0 }

func main(): i32 {
    if value() == 0 { return 0 }
    return 1
}
"#,
        ),
        (
            "isize-shift-width",
            r#"func value(): isize {
    return one() << width()
}

func one(): isize { return 1 }
func width(): isize { return 64 }

func main(): i32 {
    if value() == 0 { return 0 }
    return 1
}
"#,
        ),
    ];

    for (name, program) in cases {
        let project = TempProject::new(&format!("cli-run-{name}"));
        let source = project.write_source("invalid_integer_operation.nct", program);
        let output = nocter(&project, ["run", source.to_str().unwrap()]);
        assert!(
            !output.status.success(),
            "case {name} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            text(&output.stdout),
            text(&output.stderr)
        );
    }
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
    let value = Big { first: 1, second: 2, code: 42 }
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
fn run_command_traps_i32_unary_negate_overflow() {
    let project = TempProject::new("cli-run-i32-unary-negate-overflow");
    let source = project.write_source(
        "i32_unary_negate_overflow.nct",
        r#"func main(): i32 {
    return -minimum()
}

func minimum(): i32 {
    return -2147483648
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
fn run_command_reports_caught_direct_aggregate_call_argument_failure() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-argument-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
fn run_command_reports_caught_direct_aggregate_call_comparison_failure() {
    let project = TempProject::new("cli-run-caught-direct-aggregate-call-comparison-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
fn run_command_reports_caught_aggregate_struct_literal_field_failure() {
    let project = TempProject::new("cli-run-caught-aggregate-struct-literal-field-failure");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
    let packet = Packet {
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

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
fn run_command_traps_fixed_array_constant_index_assignment_out_of_bounds() {
    let project = TempProject::new("cli-run-fixed-array-constant-index-assignment-oob");
    let source = project.write_source(
        "fixed_array_constant_index_assignment_oob.nct",
        r#"func main(): i32 {
    var values: [i32; 1] = [0]
    values[1] = replacement()
    return values[0]
}

func replacement(): i32 {
    return 7
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
fn run_command_traps_fixed_array_variable_index_read_out_of_bounds() {
    let project = TempProject::new("cli-run-fixed-array-variable-index-read-oob");
    let source = project.write_source(
        "fixed_array_variable_index_read_oob.nct",
        r#"func main(): i32 {
    let values: [i32; 2] = [1, 2]
    let index: usize = 2
    return values[index]
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
fn run_command_traps_fixed_array_variable_index_write_out_of_bounds() {
    let project = TempProject::new("cli-run-fixed-array-variable-index-write-oob");
    let source = project.write_source(
        "fixed_array_variable_index_write_oob.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    let index: usize = 2
    values[index] = 3
    return values[0]
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
fn run_command_traps_fixed_array_variable_index_compound_assignment_out_of_bounds() {
    let project = TempProject::new("cli-run-fixed-array-variable-index-compound-oob");
    let source = project.write_source(
        "fixed_array_variable_index_compound_oob.nct",
        r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    let index: usize = 2
    values[index] += 3
    return values[0]
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

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
fn run_command_reports_fallible_entry_failure_dynamic_message() {
    let project = TempProject::new("cli-run-fallible-failure-dynamic-message");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail_dynamic.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
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
fn run_command_reports_fallible_entry_failure_error_local_dynamic_message() {
    let project = TempProject::new("cli-run-fallible-failure-error-local-dynamic-message");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail_error_local_dynamic.nct",
        r#"use std/error.Error

func main(): i32! {
    let value = Error.new("app.failed", dynamic())
    return value
}

func dynamic(): &str {
    return "failed"
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
fn run_command_reports_fully_stack_backed_error_local_failure() {
    let project = TempProject::new("cli-run-fully-stack-backed-error-local");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fully_stack_backed_error_local.nct",
        r#"use std/error.Error

func main(): i32! {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let value = Error.new(dynamic_code(), dynamic_message())
    return value
}

func dynamic_code(): &str {
    return "app.failed"
}

func dynamic_message(): &str {
    return "failed"
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
fn run_command_reports_forwarded_error_parameter_failure() {
    let project = TempProject::new("cli-run-forwarded-error-parameter");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "forwarded_error_parameter.nct",
        r#"use std/error.Error

func main(): i32! {
    return forward(Error.new("app.failed", "failed"))?
}

func forward(error: error): i32! {
    return error
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
fn run_command_reports_stack_passed_error_parameter_failure() {
    let project = TempProject::new("cli-run-stack-passed-error-parameter");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "stack_passed_error_parameter.nct",
        r#"use std/error.Error

func main(): i32! {
    return forward(1, 2, 3, 4, 5, 6, 7, 8, Error.new("app.failed", "failed"))?
}

func forward(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, error: error): i32! {
    return error
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
fn run_command_reports_split_stack_error_parameter_failure() {
    let project = TempProject::new("cli-run-split-stack-error-parameter");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "split_stack_error_parameter.nct",
        r#"use std/error.Error

func main(): i32! {
    return forward(1, 2, 3, 4, 5, 6, Error.new(dynamic_code(), dynamic_message()))?
}

func forward(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, error: error): i32! {
    return error
}

func dynamic_code(): &str {
    return "app.failed"
}

func dynamic_message(): &str {
    return "failed"
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
fn run_command_reports_fallible_entry_failure_dynamic_code_and_message() {
    let project = TempProject::new("cli-run-fallible-failure-dynamic-code-message");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "fail_dynamic_code_message.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new(dynamic_code(), dynamic_message())
}

func dynamic_code(): &str {
    return "app.failed"
}

func dynamic_message(): &str {
    return "failed"
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
fn run_command_reports_crossed_failure_payload_parameter_registers() {
    let project = TempProject::new("cli-run-crossed-failure-payload-parameter-registers");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "crossed_failure_payload_parameters.nct",
        r#"use std/error.Error

func main(): i32! {
    return fail("failed", "app.failed")?
}

func fail(message: &str, code: &str): i32! {
    return Error.new(code, message)
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
fn run_command_reports_catch_direct_error_return_failure() {
    let project = TempProject::new("cli-run-catch-direct-error-return");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "catch_direct_error_return.nct",
        r#"use std/error.Error

func main(): i32! {
    let value = answer() catch error {
        return error
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.inner: inner failed\n");
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

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
