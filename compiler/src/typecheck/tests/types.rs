use super::check_text;

#[test]
fn diagnoses_unsized_str_parameter() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func consume(text: str): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `text`"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&str"));
}

#[test]
fn diagnoses_unsized_str_parameter_without_argument_mismatch_cascade() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    consume("Nocter")
    return 0
}

func consume(text: str): void {
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `text`"));
}

#[test]
fn diagnoses_unsized_str_return_without_return_mismatch_cascade() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func title(): str {
    return "Nocter"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("function `title` return type")
    );
}

#[test]
fn diagnoses_unsized_str_return_without_missing_return_cascade() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func title(): str {
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("function `title` return type")
    );
}

#[test]
fn diagnoses_unsized_binding_annotation_without_mismatch_cascade() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let text: str = "Nocter"
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("binding `text` annotation"));
}

#[test]
fn diagnoses_unsized_conversion_target_without_conversion_cascade() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let text = "Nocter" as str
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("type conversion target"));
}

#[test]
fn diagnoses_unsized_str_optional_payload() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func maybe_title(): str? {
    return none
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("function `maybe_title` return type")
    );
    assert!(diagnostics[0].message.contains("str?"));
}

#[test]
fn diagnoses_unsized_array_fallible_success_payload() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func read_bytes(): [u8]! {
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("function `read_bytes` return type")
    );
    assert!(diagnostics[0].message.contains("[u8]!"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&[u8]"));
}

#[test]
fn diagnoses_unsized_array_struct_field() {
    let diagnostics = check_text(
        r#"struct Packet {
    bytes: [u8]
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("struct field `Packet.bytes`")
    );
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&[u8]"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&+[u8]"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("Vec<u8>"));
}

#[test]
fn diagnoses_copy_struct_field_with_non_copy_struct_type() {
    let diagnostics = check_text(
        r#"struct Text {
    ptr: *u8
    len: usize
}

copy struct Wrapper {
    text: Text
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0390");
    assert!(diagnostics[0].message.contains("Wrapper"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("Text"));
}

#[test]
fn diagnoses_copy_struct_field_with_readwrite_borrow_type() {
    let diagnostics = check_text(
        r#"copy struct Handle {
    value: &+i32
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0390");
    assert!(diagnostics[0].message.contains("&+i32"));
}

#[test]
fn accepts_copy_struct_fields_with_copy_alias_payloadless_enum_and_readonly_borrow() {
    let diagnostics = check_text(
        r#"type Count = i32

enum Mode {
    read
    write
}

copy struct Header {
    count: Count
    mode: Mode
    label: &str
    count_ref: &Count
    ptr: *u8
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_alias_to_unsized_str_in_value_position() {
    let diagnostics = check_text(
        r#"type Text = str

struct Config {
    path: Text
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("Text"));
}

#[test]
fn diagnoses_unsized_generic_argument_under_borrow() {
    let diagnostics = check_text(
        r#"struct Config {
    values: &Vec<str>
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("struct field `Config.values`")
    );
    assert!(diagnostics[0].message.contains("&Vec<str>"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&str"));
}

#[test]
fn diagnoses_unsized_generic_argument_under_slice() {
    let diagnostics = check_text(
        r#"struct Config {
    values: &[Vec<str>]
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(
        diagnostics[0]
            .message
            .contains("struct field `Config.values`")
    );
    assert!(diagnostics[0].message.contains("&[Vec<str>]"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("&str"));
}

#[test]
fn accepts_string_and_array_slice_value_types() {
    let diagnostics = check_text(
        r#"struct Config {
    path: &str
    bytes: &[u8]
    output: &+[u8]
}

func consume(path: &str, bytes: &[u8], output: &+[u8]): void {
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_raw_pointer_value_types() {
    let diagnostics = check_text(
        r#"from std/ptr import addr

struct Buffer {
    ptr: *u8
    len: usize
}

func pointer_address(pointer: *u8): usize {
    return addr(pointer)
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_raw_pointer_type_mismatch() {
    let diagnostics = check_text(
        r#"func consume(pointer: *u8): void {
}

func call_consume(pointer: *i32): void {
    consume(pointer)
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
    assert!(diagnostics[0].message.contains("*i32"));
    assert!(diagnostics[0].message.contains("*u8"));
}

#[test]
fn accepts_borrow_of_unsized_alias_as_slice() {
    let diagnostics = check_text(
        r#"type Text = str
type Bytes = [u8]

func consume(text: &Text): void {
}

func consume_bytes(bytes: &Bytes): void {
}

func main(): i32 {
    consume("Nocter")
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
