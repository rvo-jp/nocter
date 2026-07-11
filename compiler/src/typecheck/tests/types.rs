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
