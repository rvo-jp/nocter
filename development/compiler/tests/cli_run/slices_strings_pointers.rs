use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nonzero_public_pointer_from_ref_address() {
    let project = TempProject::new("cli-run-pointer-from-ref-address");
    project.write_nocter_home_file(
        "std/ptr/index.nct",
        r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
    );
    let source = project.write_source(
        "pointer_from_ref.nct",
        r#"use std/ptr as ptr

func main(): i32 {
    let byte: u8 = 1
    let address: usize = address_of(&byte)
    if address == 0 {
        return 1
    }
    return 0
}

func address_of(value: &u8): usize {
    let pointer = ptr.from_ref(value)
    return ptr.addr(pointer)
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nonzero_public_pointer_from_ref_mut_address() {
    let project = TempProject::new("cli-run-pointer-from-ref-mut-address");
    project.write_nocter_home_file(
        "std/ptr/index.nct",
        r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref_mut<T>(value: &+T): *T
"#,
    );
    let source = project.write_source(
        "pointer_from_ref_mut.nct",
        r#"use std/ptr as ptr

func main(): i32 {
    var byte: u8 = 1
    let address: usize = address_of(&+byte)
    if address == 0 {
        return 1
    }
    return 0
}

func address_of(value: &+u8): usize {
    let pointer = ptr.from_ref_mut(value)
    return ptr.addr(pointer)
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
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
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
fn run_command_returns_byte_literal_exit_code() {
    let project = TempProject::new("cli-run-byte-literal");
    let source = project.write_source(
        "byte_literal.nct",
        r#"func main(): i32 {
    let byte: u8 = b'\x41'
    return byte as i32
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
fn run_command_returns_byte_literal_comparison_exit_code() {
    let project = TempProject::new("cli-run-byte-literal-comparison");
    let source = project.write_source(
        "byte_literal_comparison.nct",
        r#"func main(): i32 {
    if b'\x41' == b'A' {
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
fn run_command_writes_reassigned_str_local() {
    let project = TempProject::new("cli-run-str-var-assignment");
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
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
fn run_command_returns_u8_arithmetic_and_shift_exit_code() {
    let project = TempProject::new("cli-run-u8-arithmetic-shift");
    let source = project.write_source(
        "u8_arithmetic_shift.nct",
        r#"func main(): i32 {
    let a: u8 = b'\x06'
    let b: u8 = b'\x03'
    let sum: u8 = a + b
    let difference: u8 = a - b
    let product: u8 = b * 4
    let quotient: u8 = a / b
    let remainder: u8 = a % 4
    let shifted_left: u8 = b << 1
    let shifted_right: u8 = a >> 1

    if sum == 9 && difference == 3 && product == 12 && quotient == 2 && remainder == 2 && shifted_left == 6 && shifted_right == 3 {
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
    return consume(Packet {
        prefix: 1,
        header: make_header(),
        tail: 99,
    })
}

func make_header(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
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
    return Pair { first: 1, second: 2 }
}

func make_wrap(): Wrap {
    return Wrap { pair: make_pair(), code: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
        "std/error/index.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
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
fn run_command_returns_stack_backed_u8_local_index_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-u8-local-index");
    let source = project.write_source(
        "stack_backed_u8_local_index.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let value: u8 = "Nocter"[0]
    return value as i32
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(78),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_stack_backed_slice_local_first_byte_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-slice-local");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "stack_backed_slice_local.nct",
        r#"use std/string.bytes

func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let view = bytes("Nocter")
    return view[0] as i32
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(78),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_split_stack_backed_str_local_first_byte_exit_code() {
    let project = TempProject::new("cli-run-split-stack-backed-str-local");
    let source = project.write_source(
        "split_stack_backed_str_local.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let text = "Nocter"
    return text[0] as i32
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(78),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_fully_stack_backed_str_local_first_byte_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-backed-str-local");
    let source = project.write_source(
        "fully_stack_backed_str_local.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let text = "Nocter"
    return text[0] as i32
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(78),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_fully_stack_backed_str_local_equality_exit_code() {
    let project = TempProject::new("cli-run-fully-stack-backed-str-local-equality");
    let source = project.write_source(
        "fully_stack_backed_str_local_equality.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let text = "Nocter"
    if text == "Nocter" && text != "Other" {
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
fn run_command_writes_str_parameter_when_len_register_aliases_destination() {
    let project = TempProject::new("cli-run-str-parameter-len-register-alias");
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

pub func write_after_two_words(first: usize, second: usize, text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    let source = project.write_source(
        "str_parameter_len_register_alias.nct",
        r#"use std/io.write_after_two_words

func main(): i32! {
    write_after_two_words(1, 2, "OK")?
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
    assert_eq!(output.stdout, b"OK");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_writes_slice_parameter_when_len_register_aliases_destination() {
    let project = TempProject::new("cli-run-slice-parameter-len-register-alias");
    project.write_nocter_home_file(
        "std/io/index.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!

pub func write_after_two_words(first: usize, second: usize, bytes: &[u8]): void! {
    write_bytes_raw(1, bytes)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "slice_parameter_len_register_alias.nct",
        r#"use std/io.write_after_two_words
use std/string.bytes

func main(): i32! {
    write_after_two_words(1, 2, bytes("OK"))?
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
    assert_eq!(output.stdout, b"OK");
    assert!(output.stderr.is_empty());
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
fn run_command_returns_str_is_empty_exit_code() {
    let project = TempProject::new("cli-run-str-is-empty");
    let source = project.write_source(
        "str_is_empty.nct",
        r#"func main(): i32 {
    if "".is_empty() == true && identity("Nocter").is_empty() == false {
        return 42
    } else {
        return 1
    }
}

func identity(text: &str): &str {
    return text
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    let packet = Packet {
        prefix: 1,
        header: make_header(),
        tail: 99,
    }
    return packet.header.code
}

func make_header(): Header {
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
    let packet = Packet {
        prefix: 1,
        header: make().header,
        tail: 99,
    }
    return packet.header.code
}

func make(): Packet {
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
fn run_command_returns_str_equality_exit_code() {
    let project = TempProject::new("cli-run-str-equality");
    let source = project.write_source(
        "str_equality.nct",
        r#"func main(): i32 {
    let same = "Nocter" == "Nocter"
    let different = "Nocter" != "Noxter"
    let empty = "" == ""
    if same && different && empty {
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
fn run_command_returns_stack_passed_str_equality_exit_code() {
    let project = TempProject::new("cli-run-stack-passed-str-equality");
    let source = project.write_source(
        "stack_passed_str_equality.nct",
        r#"func main(): i32 {
    return compare(1, 2, 3, 4, 5, 6, 7, 8, "Nocter")
}

func compare(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, text: &str): i32 {
    if text == "Nocter" && text != "Other" {
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
fn run_command_returns_value_control_struct_literal_scalar_field_exit_code() {
    let project = TempProject::new("cli-run-value-control-struct-literal-scalar-field");
    let source = project.write_source(
        "value_control_struct_literal_scalar_field.nct",
        r#"copy struct Header {
    code: i32
    tag: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let header = Header {
        code: if choice is Choice.no { 10 } else { 1 },
        tag: match choice { Choice.no { 5 } _ { 1 } },
        size: match choice { Choice.no { 7 } _ { 1 } },
        ok: if choice is Choice.no { true } else { false }
    }
    return if header.ok && header.tag == 5 && header.size == 7 {
        header.code + 32
    } else {
        1
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
fn run_command_returns_str_view_aggregate_field_exit_code() {
    let project = TempProject::new("cli-run-str-view-aggregate-field");
    let source = project.write_source(
        "str_view_aggregate_field.nct",
        r#"copy struct Label {
    text: &str
}

enum Choice {
    yes
    no
}

func make_label(text: &str): Label {
    return Label { text: text }
}

func main(): i32 {
    let choice = Choice.yes
    var label = Label { text: if choice is Choice.yes { "old" } else { "bad" } }
    if label.text != "old" {
        return 1
    }

    label.text = match choice { Choice.yes { "Nocter" } _ { "Other" } }
    if label.text != "Nocter" {
        return 2
    }

    let returned = make_label("Done")
    if returned.text == "Done" {
        return 42
    }
    return 3
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_slice_view_aggregate_field_exit_code() {
    let project = TempProject::new("cli-run-slice-view-aggregate-field");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "slice_view_aggregate_field.nct",
        r#"use std/string.bytes

copy struct Packet {
    data: &[u8]
}

enum Choice {
    yes
    no
}

func make_packet(data: &[u8]): Packet {
    return Packet { data: data }
}

func packet_data(packet: Packet): &[u8] {
    return packet.data
}

func main(): i32 {
    let choice = Choice.yes
    var packet = Packet { data: if choice is Choice.yes { bytes("Nocter") } else { bytes("x") } }
    if packet.data.len() != 6 {
        return 1
    }
    if packet.data[0] != 78 {
        return 2
    }

    let data: &[u8] = packet.data
    if data[5] != 114 {
        return 3
    }

    packet.data = match choice { Choice.yes { bytes("Done") } _ { bytes("bad") } }
    if packet.data.len() != 4 {
        return 4
    }

    let returned = make_packet(bytes("OK"))
    let returned_data = packet_data(returned)
    if returned_data[1] == 75 {
        return 42
    }
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
