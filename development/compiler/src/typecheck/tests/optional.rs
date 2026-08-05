use super::check_text;

#[test]
fn accepts_optional_none_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func lookup(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_annotated_optional_binding_from_optional_initializer() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value: i32? = maybe_answer()
    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_propagation_in_optional_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value(): i32? {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_propagation_in_fallible_optional_success_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value(): (i32?)! {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_propagation_in_non_optional_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0335");
    assert_eq!(
        diagnostics[0].help.as_deref(),
        Some(
            "handle `none` with `otherwise` or make the current callable return an optional value"
        )
    );
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, 1);
}

#[test]
fn diagnoses_optional_propagation_in_non_optional_fallible_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value(): i32! {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0335");
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, 1);
}

#[test]
fn accepts_optional_force_unwrap() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return maybe_answer()!
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_catch_on_optional_expression() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return maybe_answer() catch error {
        return 0
    }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0330");
    assert!(diagnostics[0].message.contains("catch"));
    let span = diagnostics[0].primary_span.as_ref().unwrap();
    assert_eq!(span.end_byte - span.start_byte, "catch".len());
}

#[test]
fn accepts_otherwise_early_return_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { return 1 }

    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_otherwise_break_binding_inside_loop() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var total = 0
    loop {
        let value = next(total) otherwise { break }
        total += value
    }
    return total
}

func next(total: i32): i32? {
    if total == 0 {
        return 42
    }
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_otherwise_continue_binding_inside_loop() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var index = 0
    var total = 0
    while index < 4 {
        index += 1
        let value = only_even(index) otherwise { continue }
        total += value
    }
    return total
}

func only_even(index: i32): i32? {
    if index == 2 {
        return 20
    }
    if index == 4 {
        return 22
    }
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn keeps_plain_optional_borrow_binding_as_borrow_of_optional() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    let value: &i32 = &maybe
    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("&i32?"));
}

#[test]
fn diagnoses_otherwise_non_optional_left_operand() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 1 otherwise { 2 }

    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0396");
}

#[test]
fn diagnoses_otherwise_fallback_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    maybe_answer() otherwise { "missing" }

    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0397");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_otherwise_never_terminal() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { trap() }

    return value
}

func maybe_answer(): i32? {
    return 42
}

primitive trap(): never
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn uses_otherwise_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_title() otherwise { return 1 }

    return value
}

func maybe_title(): &str? {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn accepts_otherwise_expression() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return maybe_answer() otherwise { 7 }
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_otherwise_contextual_integer_literal() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return widen(maybe_byte() otherwise { 0 })
}

func maybe_byte(): u8? {
    return 7
}

func widen(value: u8): i32 {
    return value as i32
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
