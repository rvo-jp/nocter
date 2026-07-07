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
