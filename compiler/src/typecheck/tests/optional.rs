use super::check_text;

#[test]
fn accepts_optional_none_return() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
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
fn accepts_optional_let_else_extraction() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
fn diagnoses_optional_let_else_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
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
fn uses_optional_let_else_unwrapped_return_type() {
    let diagnostics = check_text(
        r#"program(): i32 {
    let value = maybe_title() else {
        return 1
    }

    return value
}

func maybe_title(): str? {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
fn diagnoses_optional_if_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
    if let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
        r#"program(): i32 {
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
fn diagnoses_optional_while_let_non_optional_initializer() {
    let diagnostics = check_text(
        r#"program(): i32 {
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
        r#"program(): i32 {
    while let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}
