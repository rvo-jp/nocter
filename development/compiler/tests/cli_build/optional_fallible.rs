use super::*;

#[test]
fn build_command_lowers_composed_fallible_optional_scalar_call() {
    let project = TempProject::new("cli-build-composed-fallible-optional-scalar-call");
    let source = project.write_source(
        "composed_fallible_optional_scalar_call.nct",
        r#"func main(): i32! {
    let present = lookup(true)? otherwise { return 1 }
    let absent = lookup(false)? otherwise { return present }
    return absent
}

func lookup(present: bool): i32?! {
    if present { return 42 }
    return none
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fallible_aggregate_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-aggregate-binding-borrow");
    let source = project.write_source(
        "aggregate_binding_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = make()?
    touch(&+value)?
    return 0
}

func make(): Text! {
    return Text { start: 1, len: 2, capacity: 3 }
}

func touch(value: &+Text): void! {
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
fn build_command_lowers_fallible_direct_aggregate_call_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-direct-aggregate-binding-borrow");
    let source = project.write_source(
        "fallible_direct_aggregate_binding_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32! {
    var allocator = page_allocator()?
    touch(&+allocator)?
    return 0
}

func page_allocator(): Allocator! {
    return Allocator { state: 0, kind: 0 }
}

func touch(allocator: &+Allocator): void! {
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
fn build_command_lowers_fallible_direct_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-fallible-direct-aggregate-assignment-borrow");
    let source = project.write_source(
        "fallible_direct_aggregate_assignment_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32! {
    var allocator = page_allocator()
    allocator = reset_allocator()?
    touch(&+allocator)?
    return 0
}

func page_allocator(): Allocator {
    return Allocator { state: 0, kind: 0 }
}

func reset_allocator(): Allocator! {
    return Allocator { state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void! {
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
fn build_command_lowers_propagated_indirect_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-propagated-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "propagated_aggregate_call_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = make()?
    touch(&+value)?
    return 0
}

func make(): Text! {
    return Text { start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void! {
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
fn build_command_lowers_ignored_fallible_scalar_and_view_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-fallible-scalar-view-call-statement");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "ignored_fallible_scalar_view_call_statement.nct",
        r#"use std/string.bytes

func value(): i32! {
    return 1
}

func text(): &str! {
    return "ignored"
}

func data(): &[u8]! {
    return bytes("ignored")
}

func main(): void! {
    value()?
    text()?
    data()?
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
fn build_command_lowers_ignored_fallible_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-fallible-aggregate-call-statement");
    let source = project.write_source(
        "ignored_fallible_aggregate_call_statement.nct",
        r#"struct Value {
    code: i32
}

func value(): Value! {
    return Value { code: 1 }
}

func main(): void! {
    value()?
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
fn build_command_lowers_std_process_args_failure_boundary() {
    let project = TempProject::new("cli-build-process-args-failure-boundary");
    write_process_contract_std(&project);
    let source = project.write_source(
        "process_args_failure_boundary.nct",
        r#"use std/process.args

func main(): i32! {
    let values = args()?
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
fn build_command_lowers_fallible_zero_length_fixed_array_call_result() {
    let project = TempProject::new("cli-build-fallible-zero-length-fixed-array-call-result");
    let source = project.write_source(
        "fallible_zero_length_fixed_array_call_result.nct",
        r#"func main(): i32 {
    let empty: [u8; 0] = make_empty()!
    let copied: [u8; 0] = empty
    return 0
}

func make_empty(): [u8; 0]! {
    return []
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_dynamic_failure_payload() {
    let project = TempProject::new("cli-build-dynamic-failure-payload");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "dynamic_failure_payload.nct",
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_dynamic_failure_payload_code_and_message() {
    let project = TempProject::new("cli-build-dynamic-failure-payload-code-message");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "dynamic_failure_payload_code_message.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_static_error_payload_helper() {
    let project = TempProject::new("cli-build-static-error-payload-helper");
    project.write_nocter_home_file(
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(/) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    let source = project.write_source(
        "static_error_payload_helper.nct",
        r#"use std/error.Error

func main(): i32! {
    return app_failed()
}

func app_failed(): error {
    return Error.new("app.failed", "failed")
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}
