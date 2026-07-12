use super::check_text;

#[test]
fn diagnoses_binding_annotation_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let byte: u8 = 300
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("u8"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_assignment_to_let_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let count = 0
    count = 1
    return count
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("count"));
}

#[test]
fn diagnoses_assignment_to_parameter_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return id(0)
}

func id(value: i32): i32 {
    value = 1
    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_assignment_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var count: i32 = 0
    count = "hello"
    return count
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0382");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}
