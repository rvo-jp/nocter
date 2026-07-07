use super::check_text;

#[test]
fn accepts_program_i32() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_program_i32_fallible() {
    let diagnostics = check_text(
        r#"program(): i32! {
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
fn accepts_program_void() {
    let diagnostics = check_text(
        r#"program(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_main_without_program() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0301");
}

#[test]
fn diagnoses_invalid_program_return_type() {
    let diagnostics = check_text(
        r#"program(): u64 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0303");
}

#[test]
fn diagnoses_duplicate_program() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

program(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0302");
}
