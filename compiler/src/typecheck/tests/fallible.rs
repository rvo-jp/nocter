use super::*;

#[test]
fn checks_success_type_of_fallible_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(): void! {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_fail_in_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    fail error
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_code_and_message_fields() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func error_code(error: error): str {
    return error.code
}

func error_message(error: error): str {
    return error.message
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_fields_inside_catch_block() {
    let diagnostics = check_text(
        r#"program(): i32! {
    run() catch error {
        report(error.code)
        report(error.message)
        return 1
    }

    return 0
}

func run(): i32! {
    return 0
}

func report(text: str): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unknown_error_field() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func error_code(error: error): str {
    return error.raw_code
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0369");
    assert!(diagnostics[0].message.contains("raw_code"));
}

#[test]
fn diagnoses_non_error_fail_value() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(): i32! {
    fail 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("error"));
}

#[test]
fn diagnoses_fail_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: u64): i32 {
    fail error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0333");
}

#[test]
fn diagnoses_fail_type_mismatch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: str): i32! {
    fail error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("error"));
}

#[test]
fn accepts_fail_as_terminal_branch() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    if true {
        fail error
    } else {
        return 0
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn unwraps_catch_expression_success_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_propagation_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer()?
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0331");
}

#[test]
fn diagnoses_catch_on_non_fallible_expression() {
    let diagnostics = check_text(
        r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0330");
}
