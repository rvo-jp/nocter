use super::check_text;

#[test]
fn displays_fixed_array_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return "nope"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("[u8; 4]"));
}

#[test]
fn accepts_contextual_fixed_array_literal_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C, 0x46]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_fixed_array_literal_length_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C]
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("[i32; 3]"));
    assert!(diagnostics[0].message.contains("[u8; 4]"));
}

#[test]
fn accepts_contextual_fixed_array_literal_binding() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_array_literal_element_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let items = [1, "two"]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0343");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_fixed_array_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(): u8 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return header[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_view_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(bytes: [u8]): u8 {
    return bytes[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_str_index_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func first(): u8 {
    return "hello"[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_index_on_non_indexable_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let number = 1
    let byte = number[0]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0344");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_non_integer_index_value() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    let byte = header["0"]
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0345");
    assert!(diagnostics[0].message.contains("str"));
}
