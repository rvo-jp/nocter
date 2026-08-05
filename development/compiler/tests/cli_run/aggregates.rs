use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_aggregate_field_method_receiver_exit_code() {
    let project = TempProject::new("cli-run-aggregate-field-method-receiver");
    let source = project.write_source(
        "aggregate_field_method_receiver.nct",
        r#"copy struct File {
    fd: i32
}

copy struct Holder {
    tag: i32
    file: File
}

impl File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    let holder = Holder { tag: 1, file: File { fd: 41 } }
    return holder.file.value() + holder.tag
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_readwrite_aggregate_field_method_receiver_exit_code() {
    let project = TempProject::new("cli-run-readwrite-aggregate-field-method-receiver");
    let source = project.write_source(
        "readwrite_aggregate_field_method_receiver.nct",
        r#"copy struct File {
    fd: i32
}

copy struct Holder {
    tag: i32
    file: File
}

impl File {
    method &+self.bump(): void {
        self.fd += 1
        return
    }
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 40 } }
    holder.file.bump()
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
    return PairAlias { left: 20, right: 22 }
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
fn run_command_returns_imported_direct_aggregate_call_exit_code() {
    let project = TempProject::new("cli-run-imported-direct-aggregate-call");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Pair {
    pub first: i32
    pub second: i32
}

pub func make_pair(): Pair {
    return Pair { first: 7, second: 42 }
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
    return Big { first: 1, second: 2, code: 42 }
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
    return read_ninth(1, 2, 3, 4, 5, 6, 7, 8, Bytes {
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
    return read_code(1, 2, 3, 4, 5, 6, 7, 8, Big { first: 10, second: 20, code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_field_compound_assignment_exit_code() {
    let project = TempProject::new("cli-run-field-compound-assignment");
    let source = project.write_source(
        "field_compound_assignment.nct",
        r#"struct Counter {
    count: i32
    size: usize
    byte: u8
}

func main(): i32 {
    var counter = Counter { count: 40, size: 47, byte: 6 }
    counter.count += one()
    counter.count *= 2
    counter.count -= 40
    counter.count /= 2
    counter.size %= 5
    counter.byte += 3
    counter.byte *= 2
    counter.byte -= 6
    counter.byte /= 3
    counter.byte %= 4
    if counter.count == 21 && counter.size == 2 && counter.byte == 0 {
        return 42
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
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nonterminal_field_compound_assignment_exit_code() {
    let project = TempProject::new("cli-run-nonterminal-field-compound-assignment");
    let source = project.write_source(
        "nonterminal_field_compound_assignment.nct",
        r#"struct Counter {
    count: i32
    size: usize
}

func main(): i32 {
    var counter = Counter { count: 40, size: 47 }
    if true {
        counter.count += one()
    }
    if true {
        counter.size %= 5
    }
    if counter.count == 41 && counter.size == 2 {
        return 42
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
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_ignores_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-run-ignored-aggregate-call-statement");
    let source = project.write_source(
        "ignored_aggregate_call_statement.nct",
        r#"struct Value {
    code: i32
}

func main(): i32 {
    value()
    return 42
}

func value(): Value {
    return Value { code: 1 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_imported_ignored_aggregate_call_result() {
    let project = TempProject::new("cli-run-imported-ignored-aggregate-drop");
    project.write_nocter_home_file(
        "std/resource.nct",
        r#"use std/io.write_text_raw

pub struct Handle {
    fd: i32
}

impl Handle {
    drop &+self {
        write_text_raw(1, "drop\n")!
        return
    }
}

pub func make(): Handle {
    return Handle { fd: 1 }
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "imported_ignored_aggregate_drop.nct",
        r#"use std/resource.make

func main(): i32 {
    make()
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
    assert_eq!(
        output.stdout,
        b"drop\n",
        "stderr:\n{}",
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_ignores_aggregate_literal_expression_statement() {
    let project = TempProject::new("cli-run-ignored-aggregate-literal-statement");
    let source = project.write_source(
        "ignored_aggregate_literal_statement.nct",
        r#"struct Value {
    code: i32
}

func main(): i32 {
    Value { code: 1 }
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
    let result = consume(Header { tag: 7, ok: true, code: 42, len: 11 })
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
    return consume(Header { tag: 7, ok: true, code: 42, len: 11 })
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
    return consume(Pair { first: 42, second: 7 })
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
    return consume(5, Pair { a: 10, b: 20, c: 41, d: 2 }, 1)
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
    return consume(5, Big { first: 1, second: 2, code: 41 }, 1)
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
    return consume(Big { first: 10, second: 20, code: 42 })
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
    return consume(1, 2, 3, 4, 5, 6, Pair { a: 10, b: 20, c: 42, d: 7 })
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
    return consume(1, 2, 3, 4, 5, 6, 7, Big { first: 10, second: 20, code: 42 })
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
    let packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
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
    let packet = wrap(Header { tag: 7, ok: true, code: 42, len: 11 })
    return packet.header.code
}

func wrap(header: Header): Packet {
    return Packet { prefix: 1, header: header, tail: 99 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_concrete_generic_aggregate_exit_code() {
    let project = TempProject::new("cli-run-concrete-generic-aggregate");
    let source = project.write_source(
        "concrete_generic_aggregate.nct",
        r#"struct Pair<T, U> {
    first: T
    second: U
}

struct Box<T> {
    value: Pair<T, i32>
}

func main(): i32 {
    let box = make_box()
    return read(move box)
}

func make_box(): Box<i32> {
    return Box<i32> { value: Pair<i32, i32> { first: 1, second: 42 } }
}

func read(box: Box<i32>): i32 {
    return box.value.second
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
        r#"#target: "arm64-darwin"
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
    drop &+self {
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
    var file = File { fd: 3 }
    return Pair { first: 1, second: 42 }
}
"#,
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
    let result = consume(Code { value: 42 })
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
    let result = consume(Bytes { first: 7, second: 42 })
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
    let result = consume(Bytes { first: 7, second: 11, third: 42 })
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
    let result = consume(Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
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
    return consume(Bytes {
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
    return consume(5, Bytes {
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
    return consume(1, 2, 3, 4, 5, 6, Bytes {
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
    return consume(5, Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 41 }, 42)
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
    return consume(1, 2, 3, 4, 5, 6, 7, Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
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
    return Bytes { first: 7, second: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 5, sixth: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 5, sixth: 6, seventh: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Big { first: 1, second: 2, third: 3, code: 42 }
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
    return Big { first: 1, second: 2, third: 3, code: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    return Big { first: 1, second: 2, third: 3, code: 42 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 1, ok: false, code: 2, len: 3 },
        tail: 4,
    }
    packet.header = source() catch error {
        return Error.new("app.main", error.message)
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
    let text = Text { start: 1, len: 42, capacity: 99 }
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
fn run_command_returns_stack_backed_i32_local_arithmetic_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-i32-local-arithmetic");
    let source = project.write_source(
        "stack_backed_i32_local_arithmetic.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let value = 29
    let divisor = 5
    let remainder = value % divisor
    return remainder + 38
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_stack_backed_usize_local_arithmetic_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-usize-local-arithmetic");
    let source = project.write_source(
        "stack_backed_usize_local_arithmetic.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let left: usize = 100
    let right: usize = 58
    let value = left - right
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
fn run_command_returns_stack_backed_bool_local_condition_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-bool-local-condition");
    let source = project.write_source(
        "stack_backed_bool_local_condition.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let ready = "Nocter"[0] == 78
    if ready {
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
fn run_command_preserves_stack_backed_local_across_call_exit_code() {
    let project = TempProject::new("cli-run-stack-backed-local-across-call");
    let source = project.write_source(
        "stack_backed_local_across_call.nct",
        r#"func main(): i32 {
    let a0 = 1
    let a1 = 2
    let a2 = 3
    let a3 = 4
    let a4 = 5
    let a5 = 6
    let a6 = 7
    let kept = 41
    let increment = add_one(0)
    return kept + increment
}

func add_one(value: i32): i32 {
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
    let text = Text { start: 1, len: 42, capacity: 99 }
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
    let pair = Pair { a: 10, b: 20, c: 7, d: 5 }
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
    let pair = Pair { a: 10, b: 20, c: 7, d: 5 }
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
    return consume(1, 2, 3, 4, 5, 6, 7, Bytes {
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
    return consume(1, 2, 3, 4, 5, 6, 7, 8, Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 })
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
    return consume(1, 2, 3, 4, 5, 6, 7, 8, Bytes {
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
    let bytes = identity(1, 2, 3, 4, 5, 6, 7, 8, Bytes {
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
    let bytes = identity(1, 2, 3, 4, 5, 6, 7, 8, Bytes {
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
    let value = identity(1, 2, 3, 4, 5, 6, 7, 8, Big { first: 10, second: 20, code: 42 })
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
    var header = Header { tag: 7, ok: true, code: 1, len: 11 }
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
    let pair = Pair { value: 1 }
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
fn run_command_passes_aggregate_call_field_borrow_argument() {
    let project = TempProject::new("cli-run-aggregate-call-field-borrow-argument");
    let source = project.write_source(
        "aggregate_call_field_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    return choose(&make().value, 42)
}

func make(): Pair {
    return Pair { value: 1 }
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
fn run_command_passes_borrowed_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-run-borrowed-aggregate-field-borrow-argument");
    let source = project.write_source(
        "borrowed_aggregate_field_borrow_argument.nct",
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair { value: 1 }
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
    var pair = Pair { value: 1 }
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
    let packet = Packet { code: 42, len: 7, cap: 9 }
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
    var packet = Packet { code: 1, len: 7, cap: 9 }
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 1, ok: false, code: 1, len: 2 },
        tail: 3,
    }
    set_header(1, 2, 3, 4, 5, 6, 7, 8, &+packet)
    return packet.header.code
}

func set_header(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, packet: &+Packet): void {
    let ignored = noise()
    packet.header = Header { tag: 7, ok: true, code: 42, len: 11 }
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
    let packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
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
    let packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 1, len: 11 },
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    let header = Header { tag: 8, ok: true, code: 42, len: 12 }
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    let header = Header { tag: 8, ok: true, code: 42, len: 12 }
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
    drop &+self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 7 } }
    holder.file = File { fd: 41 }
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
    drop &+self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 7 } }
    replace(&+holder)
    return holder.file.fd + holder.tag
}

func replace(holder: &+Holder): void {
    holder.file = File { fd: 41 }
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
    drop &+self {
        return
    }
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 7 } }
    holder.file = make_file()
    return holder.file.fd + holder.tag
}

func make_file(): File {
    return File { fd: 41 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    var count = 0
    while count == 0 {
        file = File { fd: 41 }
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
    var bytes = Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 1 }
    let replacement = Bytes { first: 5, second: 6, third: 7, fourth: 8, fifth: 42 }
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
    var bytes = Bytes {
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
    let replacement = Bytes {
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
    let bytes = Bytes {
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make_header()
    return packet.header.code
}

func make_header(): Header {
    return Header { tag: 8, ok: true, code: 42, len: 12 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
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
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: false, code: 1, len: 11 },
        tail: 99,
    }
    packet.header = make().header
    return packet.header.code
}

func make(): Packet {
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
fn run_command_returns_value_if_aggregate_scalar_field_assignment_exit_code() {
    let project = TempProject::new("cli-run-value-if-aggregate-scalar-field-assignment");
    let source = project.write_source(
        "value_if_aggregate_scalar_field_assignment.nct",
        r#"copy struct Packet {
    count: i32
    byte: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
}

func main(): i32 {
    var packet = Packet { count: 0, byte: 0, size: 0, ok: false }
    let choice = Choice.no
    packet.count = if choice is Choice.no { 10 } else { 1 }
    packet.byte = if packet.count == 10 { 5 } else { 1 }
    packet.size = if packet.count == 10 { 7 } else { 1 }
    packet.ok = if packet.count == 10 { true } else { false }
    return if packet.ok { packet.count + 32 } else { 1 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
