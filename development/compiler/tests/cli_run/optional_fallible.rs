use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_observes_both_composed_fallible_optional_tags() {
    let project = TempProject::new("cli-run-composed-fallible-optional-tags");
    let source = project.write_source(
        "composed_fallible_optional_tags.nct",
        r#"func main(): i32! {
    let present = lookup(true)? otherwise { return 1 }
    let absent = lookup(false)? otherwise { return 42 }
    return present + absent
}

func lookup(present: bool): i32?! {
    if present { return 42 }
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
fn run_command_propagates_composed_fallible_optional_error_payload() {
    let project = TempProject::new("cli-run-composed-fallible-optional-error");
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
        "composed_fallible_optional_error.nct",
        r#"use std/error.Error

func main(): i32! {
    let value = lookup()? otherwise { return 2 }
    return value
}

func lookup(): i32?! {
    return Error.new("app.lookup", "failed")
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"app.lookup: failed\n");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_ignores_fallible_scalar_and_view_call_expression_statement() {
    let project = TempProject::new("cli-run-ignored-fallible-scalar-view-call-statement");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "ignored_fallible_scalar_view_call_statement.nct",
        r#"use std/string.bytes

func main(): i32! {
    value()?
    text()?
    data()?
    return 42
}

func value(): i32! {
    return 1
}

func text(): &str! {
    return "ignored"
}

func data(): &[u8]! {
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
fn run_command_ignores_fallible_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-run-ignored-fallible-aggregate-call-statement");
    let source = project.write_source(
        "ignored_fallible_aggregate_call_statement.nct",
        r#"copy struct Big {
    a: usize
    b: usize
    c: usize
}

func main(): i32! {
    value()?
    return 42
}

func value(): Big! {
    return Big { a: 1, b: 2, c: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
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
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Header { tag: 7, ok: true, code: 42, len: 11 }
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
    let packet = Packet {
        prefix: 1,
        header: make()!,
        tail: 99,
    }
    return packet.header.code
}

func make(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_function_inferred_from_catch_block_exit_code() {
    let project = TempProject::new("cli-run-generic-function-expected-catch-return");
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
        "generic_function_expected_catch_return.nct",
        r#"use std/error.Error

struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func source(): Marker<u8>! {
    return Error.new("app.source", "source failed")
}

func recover(): Marker<u8> {
    return source() catch error {
        return make()
    }
}

func main(): i32 {
    return recover().code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Pair { first: 7, second: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Code { value: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Bytes {
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
    return Pair { first: 7, second: 42 }
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
    return Pair { a: 10, b: 20, c: 41, d: 2 }
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
    return Big { first: 1, second: 2, code: 41 }
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
    return Pair { a: 10, b: 20, c: 42, d: 7 }
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
    return Big { first: 10, second: 20, code: 42 }
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
    return Code { value: 42 }
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
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 }
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
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 41 }
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
    return Bytes {
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
    drop &+self {
        touch2(self.fd, 99)
        return
    }
}

func main(): void! {
    var file = File { fd: 3 }
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
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32! {
    var file = File { fd: 3 }
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
fn run_command_binds_fallible_fixed_array_call_result() {
    let project = TempProject::new("cli-run-fallible-fixed-array-call-result");
    let source = project.write_source(
        "fallible_fixed_array_call_result.nct",
        r#"func main(): i32 {
    let values: [i32; 2] = make_pair()!
    return values[0] + values[1]
}

func make_pair(): [i32; 2]! {
    return [20, 22]
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_fallible_fixed_array_catch_values() {
    let project = TempProject::new("cli-run-fallible-fixed-array-catch-values");
    let source = project.write_source(
        "fallible_fixed_array_catch_values.nct",
        r#"copy struct Bag {
    values: [i32; 2]
}

func main(): i32! {
    var values: [i32; 2] = [0, 0]
    values = make_pair() catch error {
        return error
    }

    let bound: [i32; 2] = make_pair() catch error {
        return error
    }

    var bag = Bag { values: [0, 0] }
    bag.values = make_pair() catch error {
        return error
    }

    return values[0] + bound[1] + bag.values[0]
}

func make_pair(): [i32; 2]! {
    return [11, 20]
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_fixed_array_optional_otherwise_values() {
    let project = TempProject::new("cli-run-fixed-array-optional-otherwise-values");
    let source = project.write_source(
        "fixed_array_optional_otherwise_values.nct",
        r#"func main(): i32 {
    let fallback: [i32; 3] = [4, 5, 6]
    let success: [i32; 3] = maybe_values(true) otherwise { [7, 8, 9] }
    let recovered: [i32; 3] = maybe_values(false) otherwise { fallback }
    let returned: [i32; 3] = choose(false)
    return sum(success) + sum(recovered) + sum(returned)
}

func choose(flag: bool): [i32; 3] {
    return maybe_values(flag) otherwise { make_values() }
}

func maybe_values(flag: bool): [i32; 3]? {
    if flag {
        return [1, 2, 3]
    }
    return none
}

func make_values(): [i32; 3] {
    return [10, 11, 12]
}

func sum(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(54),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_fixed_array_optional_otherwise_assignments() {
    let project = TempProject::new("cli-run-fixed-array-optional-otherwise-assignments");
    let source = project.write_source(
        "fixed_array_optional_otherwise_assignments.nct",
        r#"copy struct Bag {
    tag: i32
    values: [i32; 3]
}

func main(): i32 {
    var values: [i32; 3] = [0, 0, 0]
    let fallback: [i32; 3] = [1, 2, 3]
    var bag = Bag { tag: 5, values: [0, 0, 0] }
    values = maybe_values(false) otherwise { [1, 2, 3] }
    values = maybe_values(false) otherwise { fallback }
    bag.values = maybe_values(true) otherwise { [90, 91, 92] }
    let field_success_total: i32 = sum(bag.values)
    bag.values = maybe_values(false) otherwise { make_values() }
    return sum(values) + field_success_total + sum(bag.values) + bag.tag
}

func maybe_values(flag: bool): [i32; 3]? {
    if flag {
        return [7, 8, 9]
    }
    return none
}

func make_values(): [i32; 3] {
    return [10, 11, 15]
}

func sum(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(71),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_aggregate_optional_otherwise_assignments() {
    let project = TempProject::new("cli-run-aggregate-optional-otherwise-assignments");
    let source = project.write_source(
        "aggregate_optional_otherwise_assignments.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    let fallback = Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 }
    var packet = Packet {
        prefix: 5,
        header: Header { tag: 3, ok: false, code: 3, len: 3 },
        triple: Triple { first: 1, second: 1, third: 1, fourth: 1, fifth: 1 },
    }
    header = maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    packet.header = maybe_header(true) otherwise { Header { tag: 9, ok: false, code: 90, len: 9 } }
    packet.triple = maybe_triple(false) otherwise { fallback }
    let returned = assign_with_return_fallback()
    return header_score(header) + header_score(packet.header) + triple_score(packet.triple) + returned + packet.prefix
}

func assign_with_return_fallback(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    header = maybe_header(false) otherwise { return 19 }
    return header.code
}

func header_score(header: Header): i32 {
    return header.code
}

func triple_score(triple: Triple): i32 {
    return triple.second + triple.fifth
}

func maybe_header(flag: bool): Header? {
    if flag {
        return Header { tag: 4, ok: true, code: 10, len: 4 }
    }
    return none
}

func maybe_triple(flag: bool): Triple? {
    if flag {
        return Triple { first: 3, second: 30, third: 3, fourth: 3, fifth: 3 }
    }
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(53),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_aggregate_optional_otherwise_member_roots() {
    let project = TempProject::new("cli-run-aggregate-optional-otherwise-member-roots");
    let source = project.write_source(
        "aggregate_optional_otherwise_member_roots.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    let fallback = Packet {
        prefix: 5,
        header: Header { tag: 1, ok: false, code: 7, len: 2 },
        triple: Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 },
    }
    let code = (maybe_packet(false) otherwise { fallback }).header.code
    let triple = (maybe_packet(true) otherwise { fallback }).triple
    return code + triple.second + member_return_fallback()
}

func member_return_fallback(): i32 {
    let code = (maybe_packet(false) otherwise { return 11 }).header.code
    return code
}

func maybe_packet(flag: bool): Packet? {
    if flag {
        return Packet {
            prefix: 6,
            header: Header { tag: 4, ok: true, code: 10, len: 4 },
            triple: Triple { first: 3, second: 30, third: 3, fourth: 3, fifth: 3 },
        }
    }
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(48),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_binds_fallible_zero_length_fixed_array_call_result() {
    let project = TempProject::new("cli-run-fallible-zero-length-fixed-array-call-result");
    let source = project.write_source(
        "fallible_zero_length_fixed_array_call_result.nct",
        r#"func main(): i32 {
    let empty: [u8; 0] = make_empty()!
    let copied: [u8; 0] = empty
    return 42
}

func make_empty(): [u8; 0]! {
    return []
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Pair { a: 10, b: 20, c: 7, d: 5 }
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
    return Pair { a: 10, b: 20, c: 7, d: 5 }
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
    return Big { first: 10, second: 20, code: 42 }
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
    let packet = Packet {
        prefix: 1,
        header: make_header()?,
        tail: 99,
    }
    return packet.header.code
}

func make_header(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    let packet = Packet {
        prefix: 1,
        header: make()?.header,
        tail: 99,
    }
    return packet.header.code
}

func make(): Packet! {
    return Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make()?.header
    return packet.header.code
}

func make(): Packet! {
    return Packet {
        prefix: 1,
        header: Header { tag: 8, ok: true, code: 42, len: 12 },
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
fn run_command_returns_optional_otherwise_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-return-success");
    let source = project.write_source(
        "optional_otherwise_return_success.nct",
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { return 7 }

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
fn run_command_returns_optional_otherwise_return_none_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-return-none");
    let source = project.write_source(
        "optional_otherwise_return_none.nct",
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { return 7 }

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
fn run_command_returns_optional_terminal_if_none_branch_exit_code() {
    let project = TempProject::new("cli-run-optional-terminal-if-none-branch");
    let source = project.write_source(
        "optional_terminal_if_none_branch.nct",
        r#"func main(): i32 {
    let success = maybe_answer(true) otherwise { 0 }
    let fallback = maybe_answer(false) otherwise { 2 }
    return success + fallback
}

func maybe_answer(flag: bool): i32? {
    if flag {
        return 40
    } else {
        return none
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
fn run_command_runs_optional_otherwise_never_scope_drop_before_trap() {
    let project = TempProject::new("cli-run-optional-otherwise-never-cleanup");
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
        "optional_otherwise_never_cleanup.nct",
        r#"use std/log.write
use std/process.exit

struct File {
    fd: i32
}

impl File {
    drop &+self {
        write("drop\n")!
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    let value = maybe_answer() otherwise { exit(7) }

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
fn run_command_returns_optional_otherwise_success_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-success");
    let source = project.write_source(
        "optional_otherwise_success.nct",
        r#"func main(): i32 {
    return maybe_answer() otherwise { 7 }
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
fn run_command_returns_optional_otherwise_none_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-none");
    let source = project.write_source(
        "optional_otherwise_none.nct",
        r#"func main(): i32 {
    return maybe_answer() otherwise { 7 }
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
fn run_command_returns_optional_otherwise_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-binding-success");
    let source = project.write_source(
        "optional_otherwise_binding_success.nct",
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { 7 }
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
fn run_command_returns_optional_otherwise_binding_none_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-binding-none");
    let source = project.write_source(
        "optional_otherwise_binding_none.nct",
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { 7 }
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
fn run_command_returns_optional_otherwise_break_binding_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-break-binding");
    let source = project.write_source(
        "optional_otherwise_break_binding.nct",
        r#"func main(): i32 {
    var total = 0
    loop {
        let value = next(total) otherwise { break }
        total += value
    }
    return total
}

func next(total: i32): i32? {
    if total == 0 {
        return 2
    }
    if total == 2 {
        return 40
    }
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
fn run_command_returns_optional_otherwise_continue_binding_exit_code() {
    let project = TempProject::new("cli-run-optional-otherwise-continue-binding");
    let source = project.write_source(
        "optional_otherwise_continue_binding.nct",
        r#"func main(): i32 {
    var index = 0
    var total = 0
    while index < 4 {
        index += 1
        let value = only_even(index) otherwise { continue }
        total += value
    }
    return total
}

func only_even(index: i32): i32? {
    if index == 2 {
        return 20
    }
    if index == 4 {
        return 22
    }
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
fn run_command_uses_scalar_and_view_optional_otherwise_assignments() {
    let project = TempProject::new("cli-run-scalar-view-optional-otherwise-assignments");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "scalar_view_optional_otherwise_assignments.nct",
        r#"use std/string.bytes

copy struct State {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
    data: &[u8]
}

func main(): i32 {
    var count: i32 = 0
    var byte: u8 = 0
    var size: usize = 0
    var ok: bool = false
    var text: &str = "bad"
    var data: &[u8] = bytes("bad")
    var state = State { count: 0, byte: 0, size: 0, ok: false, text: "bad", data: bytes("bad") }
    count = maybe_i32(true) otherwise { 1 }
    byte = maybe_u8(false) otherwise { 12 }
    size = maybe_usize(true) otherwise { 1 }
    ok = maybe_bool(false) otherwise { true }
    text = maybe_text(false) otherwise { "Nocter" }
    data = maybe_data(false) otherwise { bytes("*") }
    state.count = maybe_i32(false) otherwise { 5 }
    state.byte = maybe_u8(true) otherwise { 1 }
    state.size = maybe_usize(false) otherwise { 8 }
    state.ok = maybe_bool(true) otherwise { false }
    state.text = maybe_text(true) otherwise { "lang" }
    state.data = maybe_data(true) otherwise { bytes("bad") }
    let returned = assign_with_return_fallback()
    if ok && state.ok && size == 20 && state.size == 8 && text.len() == 6 && state.text.len() == 4 && data.len() == 1 && state.data.len() == 2 && data[0] == b'*' && returned == 7 {
        return count + (byte as i32) + state.count + (state.byte as i32) + 8
    }
    return 1
}

func assign_with_return_fallback(): i32 {
    var value: i32 = 0
    value = maybe_i32(false) otherwise { return 7 }
    return value
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 20 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}

func maybe_data(flag: bool): &[u8]? {
    if flag { return bytes("OK") }
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
fn run_command_uses_scalar_and_view_optional_otherwise_value_positions() {
    let project = TempProject::new("cli-run-scalar-view-optional-otherwise-values");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "scalar_view_optional_otherwise_values.nct",
        r#"use std/string.bytes

copy struct Inputs {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
    data: &[u8]
}

func main(): i32 {
    let inputs = Inputs {
        count: maybe_i32(false) otherwise { 2 },
        byte: maybe_u8(true) otherwise { 1 },
        size: maybe_usize(false) otherwise { 9 },
        ok: maybe_bool(true) otherwise { false },
        text: maybe_text(false) otherwise { "Nocter" },
        data: maybe_data(false) otherwise { bytes("*") },
    }
    let subtotal = combine(
        maybe_i32(true) otherwise { 1 },
        maybe_u8(false) otherwise { 3 },
        maybe_usize(true) otherwise { 1 },
        maybe_bool(false) otherwise { true },
        maybe_text(true) otherwise { "bad" },
        maybe_data(true) otherwise { bytes("bad") },
    )
    let branched = if false {
        maybe_i32(true) otherwise { 1 }
    } else {
        maybe_i32(false) otherwise { 4 }
    }
    let returned = return_fallback_argument()
    if inputs.ok && inputs.count == 2 && inputs.byte == 7 && inputs.size == 9 && inputs.text.len() == 6 && inputs.data.len() == 1 && inputs.data[0] == b'*' && subtotal == 33 && branched == 4 && returned == 7 {
        return 42
    }
    return 1
}

func combine(count: i32, byte: u8, size: usize, ok: bool, text: &str, data: &[u8]): i32 {
    if ok && size == 8 && text.len() == 4 && data.len() == 2 {
        return count + (byte as i32) + 20
    }
    return 1
}

func return_fallback_argument(): i32 {
    return consume_i32(maybe_i32(false) otherwise { return 7 })
}

func consume_i32(value: i32): i32 {
    return value
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 8 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}

func maybe_data(flag: bool): &[u8]? {
    if flag { return bytes("OK") }
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
fn run_command_uses_aggregate_optional_otherwise_value_arguments() {
    let project = TempProject::new("cli-run-aggregate-optional-otherwise-arguments");
    let source = project.write_source(
        "aggregate_optional_otherwise_arguments.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let direct_success = consume_header(maybe_header(true) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } })
    let direct_fallback = consume_header(maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } })
    let direct_return = fallback_return_argument()
    let indirect_success = consume_triple(maybe_triple(true) otherwise { Triple { first: 1, second: 7, third: 3 } })
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let indirect_fallback = consume_triple(maybe_triple(false) otherwise { fallback })
    let pair_success = sum_pair(maybe_pair(true) otherwise { [1, 1] })
    let pair: [i32; 2] = [2, 4]
    let pair_fallback = sum_pair(maybe_pair(false) otherwise { pair })
    let pair_literal_fallback = sum_pair(maybe_pair(false) otherwise { [3, 4] })
    return direct_success + direct_fallback + direct_return + indirect_success + indirect_fallback + pair_success + pair_fallback + pair_literal_fallback
}

func consume_header(header: Header): i32 {
    if header.ok {
        return header.code
    }
    return header.code + (header.tag as i32)
}

func consume_triple(triple: Triple): i32 {
    if triple.second == 11 {
        return 11
    }
    if triple.second == 8 {
        return 8
    }
    return 1
}

func sum_pair(pair: [i32; 2]): i32 {
    return pair[0] + pair[1]
}

func fallback_return_argument(): i32 {
    return consume_header(maybe_header(false) otherwise { return 5 })
}

func maybe_header(flag: bool): Header? {
    if flag {
        return Header { tag: 3, ok: true, code: 10, len: 1 }
    }
    return none
}

func maybe_triple(flag: bool): Triple? {
    if flag {
        return Triple { first: 1, second: 11, third: 3 }
    }
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    if flag {
        return [6, 6]
    }
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(67),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_uses_aggregate_optional_otherwise_struct_literal_fields() {
    let project = TempProject::new("cli-run-aggregate-optional-otherwise-fields");
    let source = project.write_source(
        "aggregate_optional_otherwise_fields.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

copy struct Packet {
    left: i32
    header: Header
    triple: Triple
    pair: [i32; 2]
}

func main(): i32 {
    let fallback_packet = make_packet(false)
    let success_packet = make_packet(true)
    let returned = field_return_fallback()
    return score(fallback_packet) + score(success_packet) + returned
}

func make_packet(flag: bool): Packet {
    let fallback = Triple { first: 2, second: 8, third: 4 }
    return Packet {
        left: 1,
        header: maybe_header(flag) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } },
        triple: maybe_triple(flag) otherwise { fallback },
        pair: maybe_pair(flag) otherwise { [3, 4] },
    }
}

func field_return_fallback(): i32 {
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let packet = Packet {
        left: 0,
        header: maybe_header(false) otherwise { return 5 },
        triple: maybe_triple(true) otherwise { fallback },
        pair: maybe_pair(true) otherwise { [0, 0] },
    }
    return score(packet)
}

func score(packet: Packet): i32 {
    return packet.left + header_score(packet.header) + triple_score(packet.triple) + packet.pair[0] + packet.pair[1]
}

func header_score(header: Header): i32 {
    if header.ok {
        return header.code
    }
    return header.code + (header.tag as i32)
}

func triple_score(triple: Triple): i32 {
    if triple.second == 11 {
        return 11
    }
    if triple.second == 8 {
        return 8
    }
    return 1
}

func maybe_header(flag: bool): Header? {
    if flag {
        return Header { tag: 3, ok: true, code: 10, len: 1 }
    }
    return none
}

func maybe_triple(flag: bool): Triple? {
    if flag {
        return Triple { first: 1, second: 11, third: 3 }
    }
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    if flag {
        return [6, 6]
    }
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(63),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_scalar_otherwise_bindings_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-otherwise-bindings");
    let source = project.write_source(
        "optional_scalar_otherwise_bindings.nct",
        r#"func main(): i32 {
    let byte: u8 = maybe_byte() otherwise { 1 }
    let size = maybe_size() otherwise { 2 }
    let flag = maybe_flag() otherwise { false }
    let text = maybe_text() otherwise { "fallback" }

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
fn run_command_returns_optional_scalar_otherwise_binding_fallbacks_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-otherwise-binding-fallbacks");
    let source = project.write_source(
        "optional_scalar_otherwise_binding_fallbacks.nct",
        r#"func main(): i32 {
    let byte: u8 = maybe_byte() otherwise { 42 }
    let size = maybe_size() otherwise { 40 }
    let flag = maybe_flag() otherwise { true }
    let text = maybe_text() otherwise { "fallback" }

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
fn run_command_returns_optional_scalar_otherwise_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-otherwise-return-success");
    let source = project.write_source(
        "optional_scalar_otherwise_return_success.nct",
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
    return maybe_byte() otherwise { 1 }
}

func choose_size(): usize {
    return maybe_size() otherwise { 2 }
}

func choose_flag(): bool {
    return maybe_flag() otherwise { false }
}

func choose_text(): &str {
    return maybe_text() otherwise { "fallback" }
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
fn run_command_returns_optional_scalar_otherwise_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-scalar-otherwise-return-fallback");
    let source = project.write_source(
        "optional_scalar_otherwise_return_fallback.nct",
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
    return maybe_byte() otherwise { 42 }
}

func choose_size(): usize {
    return maybe_size() otherwise { 40 }
}

func choose_flag(): bool {
    return maybe_flag() otherwise { true }
}

func choose_text(): &str {
    return maybe_text() otherwise { "fallback" }
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
fn run_command_runs_optional_scalar_otherwise_return_scope_drops() {
    let project = TempProject::new("cli-run-optional-scalar-otherwise-return-scope-drops");
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
        "optional_scalar_otherwise_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
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
    var file = File { fd: 3 }
    return maybe_answer_success() otherwise { 7 }
}

func choose_fallback(): i32 {
    var file = File { fd: 4 }
    return maybe_answer_none() otherwise { 7 }
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
fn run_command_returns_optional_direct_aggregate_otherwise_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-success");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_success.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() otherwise { return 7 }

    return header.code
}

func maybe_header(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_otherwise_none_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-none");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_none.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() otherwise { return 7 }

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
fn run_command_returns_optional_indirect_aggregate_otherwise_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-otherwise-success");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_success.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() otherwise { return 7 }

    if value.second == 42 {
        return 42
    } else {
        return 1
    }
}

func maybe_triple(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_otherwise_none_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-otherwise-none");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_none.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() otherwise { return 7 }

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
fn run_command_returns_optional_direct_aggregate_otherwise_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-binding-success");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_binding_success.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    return header.code
}

func maybe_header(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_otherwise_binding_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-binding-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_binding_fallback.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = maybe_header() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
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
fn run_command_returns_optional_direct_aggregate_otherwise_binding_copy_fallback_exit_code() {
    let project =
        TempProject::new("cli-run-optional-direct-aggregate-otherwise-binding-copy-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_binding_copy_fallback.nct",
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let fallback = Header { tag: 1, ok: false, code: 7, len: 2 }
    let header = maybe_header() otherwise { fallback }
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
fn run_command_returns_optional_indirect_aggregate_otherwise_binding_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-otherwise-binding-success");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_binding_success.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() otherwise { Triple { first: 1, second: 7, third: 3 } }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func maybe_triple(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_otherwise_binding_fallback_exit_code() {
    let project =
        TempProject::new("cli-run-optional-indirect-aggregate-otherwise-binding-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_binding_fallback.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() otherwise { Triple { first: 1, second: 7, third: 3 } }
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
fn run_command_returns_optional_indirect_aggregate_otherwise_binding_call_fallback_exit_code() {
    let project =
        TempProject::new("cli-run-optional-indirect-aggregate-otherwise-binding-call-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_binding_call_fallback.nct",
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = maybe_triple() otherwise { fallback_triple() }
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
    return Triple { first: 1, second: 7, third: 3 }
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
fn run_command_returns_optional_direct_aggregate_otherwise_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-return-success");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_return_success.nct",
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
    return maybe_header() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func maybe_header(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_direct_aggregate_otherwise_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-direct-aggregate-otherwise-return-fallback");
    let source = project.write_source(
        "optional_direct_aggregate_otherwise_return_fallback.nct",
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
    return maybe_header() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
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
fn run_command_runs_optional_direct_aggregate_otherwise_return_scope_drops() {
    let project =
        TempProject::new("cli-run-optional-direct-aggregate-otherwise-return-scope-drops");
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
        "optional_direct_aggregate_otherwise_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
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
    var file = File { fd: 3 }
    return maybe_header_success() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func choose_fallback(): Header {
    var file = File { fd: 4 }
    return maybe_header_none() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func maybe_header_success(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
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
fn run_command_returns_optional_indirect_aggregate_otherwise_return_success_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-otherwise-return-success");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_return_success.nct",
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
    return maybe_triple() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func maybe_triple(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_optional_indirect_aggregate_otherwise_return_fallback_exit_code() {
    let project = TempProject::new("cli-run-optional-indirect-aggregate-otherwise-return-fallback");
    let source = project.write_source(
        "optional_indirect_aggregate_otherwise_return_fallback.nct",
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
    return maybe_triple() otherwise { Triple { first: 1, second: 7, third: 3 } }
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
fn run_command_runs_optional_indirect_aggregate_otherwise_return_scope_drops() {
    let project =
        TempProject::new("cli-run-optional-indirect-aggregate-otherwise-return-scope-drops");
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
        "optional_indirect_aggregate_otherwise_return_scope_drops.nct",
        r#"use std/log.write

struct File {
    fd: i32
}

impl File {
    drop &+self {
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
    var file = File { fd: 3 }
    return maybe_triple_success() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func choose_fallback(): Triple {
    var file = File { fd: 4 }
    return maybe_triple_none() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func maybe_triple_success(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
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
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
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
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
