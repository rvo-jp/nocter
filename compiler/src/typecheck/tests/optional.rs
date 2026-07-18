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
fn accepts_optional_let_else_extraction() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_answer() else {
        return 1
    }

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
fn accepts_readonly_optional_let_else_projection() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    let value = &maybe else {
        return 0
    }

    return read(value)
}

func maybe_answer(): i32? {
    return 42
}

func read(value: &i32): i32 {
    return 1
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
fn accepts_readwrite_optional_let_else_projection() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    var value = &+maybe else {
        return 0
    }

    write(value)
    return 0
}

func maybe_answer(): i32? {
    return 42
}

func write(value: &+i32): void {
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_let_else_projection_annotation_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    let value: &u8 = &maybe else {
        return 0
    }
    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("&i32"));
    assert!(diagnostics[0].message.contains("&u8"));
}

#[test]
fn diagnoses_optional_let_else_projection_binding_kind_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    let value = &+maybe else {
        return 0
    }
    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0429");
    assert!(diagnostics[0].message.contains("readwrite"));
}

#[test]
fn diagnoses_optional_let_else_non_optional_initializer() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 1 else {
        return 1
    }

    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0340");
}

#[test]
fn diagnoses_optional_let_else_fallthrough() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_answer() else {
        log_missing()
    }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func log_missing(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0341");
}

#[test]
fn accepts_optional_let_else_never_terminal() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_answer() else {
        trap()
    }

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
fn uses_optional_let_else_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = maybe_title() else {
        return 1
    }

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
fn accepts_optional_if_let_extraction() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if let value = maybe_answer() {
        return value
    } else {
        return 0
    }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_if_var_extraction() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if var value = maybe_answer() {
        return value
    }

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
fn accepts_readonly_optional_if_let_projection() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    if let value = &maybe {
        return read(value)
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}

func read(value: &i32): i32 {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_optional_if_var_projection() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    if var value = &+maybe {
        write(value)
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}

func write(value: &+i32): void {
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_if_let_projection_binding_kind_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    if var value = &maybe {
        return 1
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0429");
    assert!(diagnostics[0].message.contains("readonly"));
}

#[test]
fn diagnoses_optional_if_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if let value = 1 {
        return value
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0356");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn uses_optional_if_let_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if let value = maybe_title() {
        return value
    }

    return 0
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
fn accepts_else_if_let_terminal_chain() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if false {
        return 0
    } else if let value = maybe_answer() {
        return value
    } else if var fallback = maybe_fallback() {
        return fallback
    } else {
        return 3
    }
}

func maybe_answer(): i32? {
    return none
}

func maybe_fallback(): i32? {
    return 2
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_else_if_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if false {
        return 0
    } else if let value = 1 {
        return value
    } else {
        return 2
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0356");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_optional_while_let_extraction() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while let value = maybe_answer() {
        return value
    }

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
fn accepts_optional_while_var_extraction() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while var value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_borrowed_optional_while_let_projection_deferred() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var maybe = maybe_answer()
    while let value = &maybe {
        return 1
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0430");
    assert!(diagnostics[0].message.contains("while let"));
}

#[test]
fn diagnoses_optional_while_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while let value = 1 {
        return value
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0358");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn uses_optional_while_let_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while let value = maybe_title() {
        return value
    }

    return 0
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
fn accepts_optional_default_expression() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return maybe_answer() ?? 7
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_optional_default_contextual_integer_literal() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return widen(maybe_byte() ?? 0)
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

#[test]
fn diagnoses_optional_default_non_optional_left_operand() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 1 ?? 2
    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0396");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_optional_default_fallback_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    maybe_answer() ?? "missing"
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
