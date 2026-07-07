use super::{check_text, check_text_with_entry};

#[test]
fn accepts_default_main_i32() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_default_main_i32_fallible() {
    let diagnostics = check_text(
        r#"func main(): i32! {
    return run()?
}

func run(): i32! {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_default_main_void() {
    let diagnostics = check_text(
        r#"func main(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_missing_default_main() {
    let diagnostics = check_text(
        r#"func start(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0300");
}

#[test]
fn diagnoses_invalid_default_main_return_type() {
    let diagnostics = check_text(
        r#"func main(): u64 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0303");
}

#[test]
fn diagnoses_duplicate_default_main_as_duplicate_function_name() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func main(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0400");
}

#[test]
fn diagnoses_default_main_with_parameters() {
    let diagnostics = check_text(
        r#"func main(args: str): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0303");
}

#[test]
fn accepts_configured_entry_name() {
    let diagnostics = check_text_with_entry(
        r#"func start(): i32 {
    return 0
}
"#,
        "start",
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn configured_entry_name_does_not_fall_back_to_main() {
    let diagnostics = check_text_with_entry(
        r#"func main(): i32 {
    return 0
}
"#,
        "start",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0300");
}
