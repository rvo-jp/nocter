use super::check_text;

#[test]
fn checks_success_type_of_fallible_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
fn accepts_error_return_in_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(error: error): i32! {
    return error
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_code_and_message_fields() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func error_code(error: error): &str {
    return error.code
}

func error_message(error: error): &str {
    return error.message
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_error_fields_inside_catch_block() {
    let diagnostics = check_text(
        r#"func main(): i32! {
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

func report(text: &str): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unknown_error_field() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func error_code(error: error): &str {
    return error.raw_code
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0369");
    assert!(diagnostics[0].message.contains("raw_code"));
}

#[test]
fn diagnoses_non_success_return_value() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(): i32! {
    return "wrong"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_error_return_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(error: error): i32 {
    return error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
}

#[test]
fn diagnoses_error_success_type_in_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(error: error): error! {
    return error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0334");
    assert!(diagnostics[0].message.contains("error!"));
}

#[test]
fn diagnoses_error_type_mismatch_in_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(error: &str): i32! {
    return error
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_error_return_as_terminal_branch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func run(error: error): i32! {
    if true {
        return error
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
        r#"func main(): i32 {
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
fn accepts_fallible_force_unwrap() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return answer()!
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_force_unwrap_on_plain_value() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return answer()!
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0336");
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, 1);
}

#[test]
fn diagnoses_propagation_in_non_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return answer()?
}

func answer(): i32! {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0331");
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, 1);
}

#[test]
fn diagnoses_catch_on_non_fallible_expression() {
    let diagnostics = check_text(
        r#"func main(): i32 {
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
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, "catch".len());
}
