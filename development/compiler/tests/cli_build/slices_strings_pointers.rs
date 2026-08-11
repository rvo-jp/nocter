use super::*;

#[test]
fn build_command_lowers_byte_literal() {
    let project = TempProject::new("cli-build-byte-literal");
    let source = project.write_source(
        "byte_literal.nct",
        r#"func main(): i32 {
    let byte: u8 = b'\x41'
    return byte as i32
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_u8_arithmetic_and_shifts() {
    let project = TempProject::new("cli-build-u8-arithmetic-shifts");
    let source = project.write_source(
        "u8_arithmetic_shifts.nct",
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
fn build_command_lowers_nonterminal_slice_index_assignments() {
    let project = TempProject::new("cli-build-nonterminal-slice-index-assignments");
    let source = project.write_source(
        "nonterminal_slice_index_assignments.nct",
        r#"func main(): i32 {
    let bytes = buffer()
    if true {
        bytes[0] = 1
    }
    while false {
        bytes[1] = 2
    }
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_slice_index_compound_assignments() {
    let project = TempProject::new("cli-build-slice-index-compound-assignments");
    let source = project.write_source(
        "slice_index_compound_assignments.nct",
        r#"func main(): i32 {
    let numbers = i32_buffer()
    numbers[0] += 1
    let words = usize_buffer()
    words[1] %= 5
    return 0
}

func i32_buffer(): &+[i32] {
    return i32_buffer()
}

func usize_buffer(): &+[usize] {
    return usize_buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_nonterminal_slice_index_compound_assignments() {
    let project = TempProject::new("cli-build-nonterminal-slice-index-compound-assignments");
    let source = project.write_source(
        "nonterminal_slice_index_compound_assignments.nct",
        r#"func main(): i32 {
    let numbers = i32_buffer()
    if true {
        numbers[0] += 1
    }
    let words = usize_buffer()
    while false {
        words[1] %= 5
    }
    return 0
}

func i32_buffer(): &+[i32] {
    return i32_buffer()
}

func usize_buffer(): &+[usize] {
    return usize_buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_u8_slice_index_compound_assignment() {
    let project = TempProject::new("cli-build-u8-slice-index-compound-assignment");
    let source = project.write_source(
        "u8_slice_index_compound_assignment.nct",
        r#"func main(): i32 {
    let bytes = buffer()
    bytes[0] += 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_imported_alias_slice_call_result_compound_assignment() {
    let project = TempProject::new("cli-build-imported-alias-slice-call-result-compound");
    project.write_source(
        "slice_api/index.nct",
        r#"pub type MutBytes = &+[u8]

pub func buffer(): MutBytes {
    return buffer()
}
"#,
    );
    let source = project.write_source(
        "imported_alias_slice_call_result_compound.nct",
        r#"use ./slice_api.buffer

func main(): i32 {
    buffer()[0] += 1
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
fn build_command_lowers_imported_alias_slice_call_result_borrow_argument() {
    let project = TempProject::new("cli-build-imported-alias-slice-call-result-borrow");
    project.write_source(
        "slice_api/index.nct",
        r#"pub type MutBytes = &+[u8]

pub func buffer(): MutBytes {
    return buffer()
}
"#,
    );
    let source = project.write_source(
        "imported_alias_slice_call_result_borrow.nct",
        r#"use ./slice_api.buffer

func main(): i32 {
    touch(&+buffer()[0])
    return 0
}

func touch(byte: &+u8): void {
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
fn build_command_lowers_aggregate_struct_literal_binding_and_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-literal-binding-borrow");
    let source = project.write_source(
        "aggregate_literal_binding_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text { start: 1, len: 2, capacity: 3 }
    touch(&+value)?
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
fn build_command_lowers_aggregate_struct_literal_assignment_and_borrow_argument() {
    let project = TempProject::new("cli-build-aggregate-literal-assignment-borrow");
    let source = project.write_source(
        "aggregate_literal_assignment_borrow.nct",
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32! {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = Text { start: 4, len: 5, capacity: 6 }
    touch(&+value)?
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
fn build_command_lowers_moved_aggregate_struct_literal_field() {
    let project = TempProject::new("cli-build-moved-aggregate-struct-literal-field");
    let source = project.write_source(
        "moved_aggregate_struct_literal_field.nct",
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Holder {
    file: File
}

destruct Holder(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 7 }
    var holder = Holder { file: move file }
    return holder.file.fd
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_concrete_generic_struct_literal() {
    let project = TempProject::new("cli-build-concrete-generic-struct-literal");
    let source = project.write_source(
        "concrete_generic_struct_literal.nct",
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32> {
        value: 42,
    }
    return box.value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_member_rooted_slice_index_assignment() {
    let project = TempProject::new("cli-build-member-rooted-slice-index-assignment");
    project.write_nocter_home_file(
        "std/ptr/index.nct",
        r#"pub(/) primitive from_addr<T>(address: usize): *T
pub(/) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
    );
    project.write_nocter_home_file(
        "std/buffer/index.nct",
        r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func buffer(): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(1), 0)
}
"#,
    );
    let source = project.write_source(
        "index.nct",
        r#"use std/buffer.buffer

struct Buffer {
    pub bytes: &+[u8]
}

func main(): i32 {
    let holder = Buffer { bytes: buffer() }
    holder.bytes[0] = 1
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
fn build_command_lowers_public_pointer_from_ref_address_conversion() {
    let project = TempProject::new("cli-build-pointer-from-ref-address");
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
    return 0
}

func address_of(value: &u8): usize {
    let pointer = ptr.from_ref(value)
    return ptr.addr(pointer)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_public_pointer_from_ref_mut_address_conversion() {
    let project = TempProject::new("cli-build-pointer-from-ref-mut-address");
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
    return 0
}

func address_of(value: &+u8): usize {
    let pointer = ptr.from_ref_mut(value)
    return ptr.addr(pointer)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_str_equality() {
    let project = TempProject::new("cli-build-str-equality");
    let source = project.write_source(
        "str_equality.nct",
        r#"func main(): i32 {
    if "a" == "b" {
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
fn build_command_lowers_str_view_aggregate_fields() {
    let project = TempProject::new("cli-build-str-view-aggregate-fields");
    let source = project.write_source(
        "str_view_aggregate_fields.nct",
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
        return 0
    }
    return 3
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_slice_view_aggregate_fields() {
    let project = TempProject::new("cli-build-slice-view-aggregate-fields");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(/) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "slice_view_aggregate_fields.nct",
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
        return 0
    }
    return 5
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
fn built_executable_runs_str_len_and_index_call_results() {
    let project = TempProject::new("cli-build-run-str-call-result-ops");
    let source = project.write_source(
        "str_call_result_ops.nct",
        r#"func main(): i32 {
    let size: usize = identity("Nocter").len()
    let byte: u8 = identity("Nocter")[3]
    if size == 6 && byte == 116 {
        return 0
    } else {
        return 1
    }
}

func identity(text: &str): &str {
    return text
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_runs_str_is_empty_call_results() {
    let project = TempProject::new("cli-build-run-str-is-empty-call-results");
    let source = project.write_source(
        "str_is_empty_call_results.nct",
        r#"func main(): i32 {
    let empty = "".is_empty()
    let nonempty = identity("Nocter").is_empty()
    if empty && !nonempty {
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn built_executable_passes_partial_direct_aggregate_argument_bytes() {
    let project = TempProject::new("cli-build-run-partial-direct-aggregate-argument-bytes");
    let source = project.write_source(
        "partial_direct_aggregate_argument_bytes.nct",
        r#"copy struct Bytes {
    first: u8
    second: u8
    third: u8
}

func main(): i32 {
    var bytes = Bytes { first: 1, second: 2, third: 42 }
    return read(bytes) as i32
}

func read(bytes: Bytes): u8 {
    return bytes.third
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    let status = Command::new(&executable).status().unwrap();
    assert_eq!(status.code(), Some(42));
}
