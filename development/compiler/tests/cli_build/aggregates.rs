use super::*;

#[test]
fn build_command_lowers_nonterminal_if_distinct_branch_aggregate_layouts() {
    let project = TempProject::new("cli-build-nonterminal-if-distinct-branch-layouts");
    let source = project.write_source(
        "nonterminal_if_distinct_branch_layouts.nct",
        r#"struct Small {
    value: i32
}

impl Small {
    drop &+self {
        return
    }
}

struct Wide {
    left: i32
    right: i32
}

impl Wide {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var small = Small { value: 1 }
    } else {
        var wide = Wide { left: 2, right: 3 }
    }
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
fn build_command_lowers_nonterminal_while_body_local_aggregate_replacement() {
    let project = TempProject::new("cli-build-nonterminal-while-body-local-aggregate-replacement");
    let source = project.write_source(
        "nonterminal_while_body_local_aggregate_replacement.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        file = File { fd: 2 }
    }
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
fn build_command_lowers_direct_aggregate_terminal_if_return() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-return");
    let source = project.write_source(
        "direct_aggregate_terminal_if_return.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        return Pair { first: 42, second: 1 }
    } else {
        return Pair { first: 7, second: 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_call_return() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-call-return");
    let source = project.write_source(
        "direct_aggregate_terminal_if_call_return.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func make_pair(first: i32, second: i32): Pair {
    return Pair { first: first, second: second }
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        return make_pair(42, 1)
    } else {
        return make_pair(7, 2)
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_branch_leading_statements() {
    let project =
        TempProject::new("cli-build-direct-aggregate-terminal-if-branch-leading-statements");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_leading_statements.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        drop file
        return Pair { first: 42, second: 1 }
    } else {
        touch(&+file)
        return Pair { first: 7, second: 2 }
    }
}

func touch(file: &+File): void {
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
fn build_command_lowers_direct_aggregate_terminal_if_branch_local_binding() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-branch-local-binding");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_local_binding.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: i32
    second: i32
}

func main(): i32 {
    let pair = choose(true)
    return pair.first
}

func choose(flag: bool): Pair {
    if flag {
        var file = File { fd: 1 }
        return Pair { first: 42, second: 1 }
    } else {
        var file = File { fd: 2 }
        return Pair { first: 7, second: 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_direct_aggregate_terminal_if_branch_assignment() {
    let project = TempProject::new("cli-build-direct-aggregate-terminal-if-branch-assignment");
    let source = project.write_source(
        "direct_aggregate_terminal_if_branch_assignment.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = choose(true)
    drop file
    return 0
}

func choose(flag: bool): File {
    var file = File { fd: 1 }
    if flag {
        file = File { fd: 2 }
        return move file
    } else {
        file = File { fd: 3 }
        return move file
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_terminal_if_aggregate_scalar_field_short_circuit() {
    let project = TempProject::new("cli-build-terminal-if-aggregate-scalar-field-short-circuit");
    let source = project.write_source(
        "terminal_if_aggregate_scalar_field_short_circuit.nct",
        r#"struct Header {
    tag: u8
    ok: bool
}

func main(): i32 {
    let header = Header { tag: 7, ok: true }
    if header.ok && header.tag == 7 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_compound_bool_equality_in_terminal_aggregate_branch_binding() {
    let project = TempProject::new("cli-build-terminal-aggregate-branch-binding-span");
    let source = project.write_source(
        "terminal_aggregate_branch_binding_span.nct",
        r#"func main(): i32 {
    return make(true).len
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func make(flag: bool): Text {
    if flag {
        let ok = true
        let same = !ok == flag
        return Text { start: 1, len: 42, capacity: 99 }
    } else {
        return Text { start: 2, len: 7, capacity: 11 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_field_compound_assignment() {
    let project = TempProject::new("cli-build-field-compound-assignment");
    let source = project.write_source(
        "field_compound_assignment.nct",
        r#"struct Counter {
    value: i32
}

func main(): i32 {
    var counter = Counter { value: 1 }
    counter.value += 1
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
fn build_command_lowers_nonterminal_field_compound_assignments() {
    let project = TempProject::new("cli-build-nonterminal-field-compound-assignment");
    let source = project.write_source(
        "nonterminal_field_compound_assignment.nct",
        r#"struct Counter {
    count: i32
    size: usize
}

func main(): i32 {
    var counter = Counter { count: 40, size: 47 }
    if true {
        counter.count += 1
    }
    while false {
        counter.size %= 5
    }
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
fn build_command_lowers_nested_concrete_generic_aggregate_field() {
    let project = TempProject::new("cli-build-nested-concrete-generic-aggregate-field");
    let source = project.write_source(
        "nested_concrete_generic_aggregate_field.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_field_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-field-borrow-argument");
    let source = project.write_source(
        "aggregate_field_borrow_argument.nct",
        r#"type IntRef = &i32

copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair { value: 1 }
    return choose(&pair.value, 0)
}

func choose(value: IntRef, fallback: i32): i32 {
    return fallback
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_u16_u32_aggregate_scalar_fields() {
    let project = TempProject::new("cli-build-u16-u32-aggregate-scalar-fields");
    let source = project.write_source(
        "u16_u32_aggregate_scalar_fields.nct",
        r#"struct Header {
    tag: u8
    code: u16
    wide: u32
}

func main(): i32 {
    let header = make()
    return 0
}

func make(): Header {
    return Header { tag: 7, code: 42, wide: 100 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_moved_aggregate_slot_assignment() {
    let project = TempProject::new("cli-build-moved-aggregate-slot-assignment");
    let source = project.write_source(
        "moved_aggregate_slot_assignment.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var source = File { fd: 7 }
    var target = File { fd: 1 }
    target = move source
    return target.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_moved_aggregate_binding() {
    let project = TempProject::new("cli-build-moved-aggregate-binding");
    let source = project.write_source(
        "moved_aggregate_binding.nct",
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var source = File { fd: 7 }
    var target = move source
    return target.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_binding() {
    let project = TempProject::new("cli-build-copy-aggregate-binding");
    let source = project.write_source(
        "copy_aggregate_binding.nct",
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let source = Pair { left: 40, right: 2 }
    let target = source
    return target.left + target.right
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_field_from_non_copy_owner() {
    let project = TempProject::new("cli-build-copy-aggregate-field-non-copy-owner");
    let source = project.write_source(
        "copy_aggregate_field_non_copy_owner.nct",
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func main(): i32 {
    let packet = Packet { prefix: 1, header: Header { code: 40, len: 2 }, tail: 3 }
    let header = packet.header
    return header.code + header.len
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_field_from_non_copy_call_result() {
    let project = TempProject::new("cli-build-copy-aggregate-field-non-copy-call-result");
    let source = project.write_source(
        "copy_aggregate_field_non_copy_call_result.nct",
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func make_packet(): Packet {
    return Packet { prefix: 1, header: Header { code: 40, len: 2 }, tail: 3 }
}

func main(): i32 {
    let header = make_packet().header
    let again = header
    return again.code + again.len
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_copy_aggregate_slot_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-copy-aggregate-slot-assignment-borrow");
    let source = project.write_source(
        "copy_aggregate_slot_assignment_borrow.nct",
        r#"copy struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var source = Text { start: 1, len: 2, capacity: 3 }
    var target = Text { start: 4, len: 5, capacity: 6 }
    target = source
    touch(&+target)?
    return 0
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
fn build_command_lowers_imported_copy_aggregate_slot_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-imported-copy-aggregate-slot-assignment-borrow");
    project.write_nocter_home_file(
        "std/text.nct",
        r#"pub copy struct Text {
    pub start: usize
    pub len: usize
    pub capacity: usize
}
"#,
    );
    let source = project.write_source(
        "imported_copy_aggregate_slot_assignment_borrow.nct",
        r#"use std/text.Text

func main(): i32! {
    var source = Text { start: 1, len: 2, capacity: 3 }
    var target = Text { start: 4, len: 5, capacity: 6 }
    target = source
    touch(&+target)?
    return 0
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
fn build_command_lowers_direct_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-direct-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "direct_aggregate_call_assignment_borrow.nct",
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    var allocator = page_allocator()
    allocator = reset_allocator()
    touch(&+allocator)
    return 0
}

func page_allocator(): Allocator {
    return Allocator { state: 0, kind: 0 }
}

func reset_allocator(): Allocator {
    return Allocator { state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void {
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
fn build_command_lowers_indirect_aggregate_call_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-indirect-aggregate-call-assignment-borrow");
    let source = project.write_source(
        "indirect_aggregate_call_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = make()
    touch(&+value)
    return 0
}

func make(): Text {
    return Text { start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void {
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
fn build_command_lowers_std_page_allocator_direct_aggregate_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-page-allocator-borrow");
    project.write_nocter_home_file(
        "std/mem.nct",
        r#"pub struct Allocator {
    state: usize
    kind: u64
}

pub func page_allocator(): Allocator {
    return Allocator { state: 0, kind: 0 }
}
"#,
    );
    let source = project.write_source(
        "page_allocator_borrow.nct",
        r#"use std/mem.Allocator
use std/mem.page_allocator

func main(): i32 {
    var allocator = page_allocator()
    touch(&+allocator)
    return 0
}

func touch(allocator: &+Allocator): void {
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
fn build_command_lowers_ignored_aggregate_call_expression_statement() {
    let project = TempProject::new("cli-build-ignored-aggregate-call-statement");
    let source = project.write_source(
        "ignored_aggregate_call_statement.nct",
        r#"struct Value {
    code: i32
}

func value(): Value {
    return Value { code: 1 }
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
fn build_command_lowers_ignored_aggregate_literal_expression_statement() {
    let project = TempProject::new("cli-build-ignored-aggregate-literal-statement");
    let source = project.write_source(
        "ignored_aggregate_literal_statement.nct",
        r#"struct Value {
    code: i32
}

func main(): void {
    Value { code: 1 }
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
fn build_command_accepts_value_if_aggregate_scalar_field_assignments() {
    let project = TempProject::new("cli-build-value-if-aggregate-scalar-field-assignments");
    let source = project.write_source(
        "value_if_aggregate_scalar_field_assignments.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_direct_aggregate_argument_words() {
    let project = TempProject::new("cli-build-run-direct-aggregate-argument-words");
    let source = project.write_source(
        "direct_aggregate_argument_words.nct",
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    var pair = Pair { a: 10, b: 20, c: 7, d: 5 }
    return check(pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_moved_non_copy_direct_aggregate_argument() {
    let project = TempProject::new("cli-build-run-moved-non-copy-direct-aggregate-argument");
    let source = project.write_source(
        "moved_non_copy_direct_aggregate_argument.nct",
        r#"struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    let pair = Pair { a: 10, b: 20, c: 7, d: 5 }
    return check(move pair)
}

func check(pair: Pair): i32 {
    return pair.a + pair.b + pair.c + pair.d
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_returns_direct_aggregate_with_scalar_fields() {
    let project = TempProject::new("cli-build-run-direct-aggregate-scalar-return");
    let source = project.write_source(
        "direct_aggregate_scalar_return.nct",
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
}

func main(): i32 {
    var header = make()
    if header.ok {
        return header.code
    } else {
        return 1
    }
}

func make(): Header {
    return Header { tag: 7, ok: true, code: 42 }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}
