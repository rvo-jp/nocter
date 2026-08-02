use super::check_text;

#[test]
fn optional_otherwise_binding_retains_the_success_borrow_source() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func maybe(value: &Text): (&Text)? {
    return value
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let borrowed = maybe(&text) otherwise { return 0 }
    text = Text { value: 2 }
    inspect(borrowed)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("borrowed"));
}

#[test]
fn borrow_alias_binding_retains_the_original_source() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let first = &text
    let second = first
    text = Text { value: 2 }
    inspect(second)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("second"));
}

#[test]
fn value_fallback_binding_retains_every_possible_borrow_source() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func maybe(value: &Text, present: bool): (&Text)? {
    if present {
        return value
    }
    return none
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var left = Text { value: 1 }
    var right = Text { value: 2 }
    let borrowed = maybe(&left, false) otherwise { &right }
    left = Text { value: 3 }
    right = Text { value: 4 }
    inspect(borrowed)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E0434")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("left"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("right"))
    );
}
