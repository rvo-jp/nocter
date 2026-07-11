use super::check_text;

#[test]
fn diagnoses_string_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_bare_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0310");
}

#[test]
fn diagnoses_value_return_from_void_function() {
    let diagnostics = check_text(
        r#"func main(): void {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0311");
}

#[test]
fn diagnoses_missing_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_terminal_never_expression_statement() {
    let diagnostics = check_text(
        r#"primitive trap(): never

func main(): i32 {
    trap()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_from_never_function() {
    let diagnostics = check_text(
        r#"primitive trap(): never

func main(): i32 {
    return 0
}

func stop(): never {
    return trap()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0314");
}

#[test]
fn accepts_str_function_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func title(): str {
    return "hello"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_bool_function_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
