use super::check_text;

#[test]
fn accepts_supported_string_interpolation_parts() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: [u8]
}

func main(): i32 {
    return 0
}

func message(name: str, count: i32, ready: bool, owned: String): String! {
    return "name ${name} count ${count} ready ${ready} owned ${owned}"?
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unsupported_string_interpolation_part_type() {
    let diagnostics = check_text(
        r#"struct String {
    bytes: [u8]
}

func main(): i32 {
    return 0
}

func message(values: [i32]): String! {
    return "values ${values}"?
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0379");
    assert!(diagnostics[0].message.contains("[i32]"));
}
