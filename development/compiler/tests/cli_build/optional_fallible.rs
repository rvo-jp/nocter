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
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

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
fn build_command_lowers_scalar_and_view_optional_otherwise_assignments() {
    let project = TempProject::new("cli-build-scalar-view-optional-otherwise-assignments");
    project.write_nocter_home_file(
        "std/string/index.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_scalar_and_view_optional_otherwise_value_positions() {
    let project = TempProject::new("cli-build-scalar-view-optional-otherwise-values");
    project.write_nocter_home_file(
        "std/string/index.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_optional_otherwise_value_arguments() {
    let project = TempProject::new("cli-build-aggregate-optional-otherwise-arguments");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_optional_otherwise_struct_literal_fields() {
    let project = TempProject::new("cli-build-aggregate-optional-otherwise-fields");
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
fn build_command_lowers_fixed_array_optional_otherwise_values() {
    let project = TempProject::new("cli-build-fixed-array-optional-otherwise-values");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_fixed_array_optional_otherwise_assignments() {
    let project = TempProject::new("cli-build-fixed-array-optional-otherwise-assignments");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_optional_otherwise_assignments() {
    let project = TempProject::new("cli-build-aggregate-optional-otherwise-assignments");
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_aggregate_optional_otherwise_member_roots() {
    let project = TempProject::new("cli-build-aggregate-optional-otherwise-member-roots");
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

pub(nocter) primitive new_error(code: &str, message: &str): error

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

pub(nocter) primitive new_error(code: &str, message: &str): error

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

pub(nocter) primitive new_error(code: &str, message: &str): error

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
