use super::check_text;

#[test]
fn accepts_if_else_return_as_terminal_statement() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_condition_from_bool_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let enabled = true
    if enabled {
        return 0
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_condition_from_comparison() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let count = 1
    if count > 0 {
        return 0
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_bool_if_condition() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if 1 {
        return 0
    }
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0346");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_if_without_else_as_non_terminal() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    if true {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_while_bool_condition() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while ready() {
        tick()
    }

    return 0
}

func ready(): bool {
    return false
}

func tick(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_bool_while_condition() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    while 1 {
        return 0
    }

    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0357");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_break_and_continue_inside_loops() {
    let diagnostics = check_text(
        r#"func main(): void {
    while ready() {
        break
    }

    while let value = maybe_answer() {
        continue
    }
}

func ready(): bool {
    return true
}

func maybe_answer(): i32? {
    return none
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_inside_loop_expression_catch_block() {
    let diagnostics = check_text(
        r#"func main(): void {
    while ready() {
        let value = fallible() catch error {
            break
        }
    }
}

func ready(): bool {
    return true
}

func fallible(): i32! {
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_and_continue_inside_loop_statement() {
    let diagnostics = check_text(
        r#"func main(): void {
    loop {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_range_for_integer_bounds() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    for i in 0..<4 {
        return i
    }

    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_range_for_contextual_integer_literal_bound() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func first(limit: u64): u64 {
    for i in 0..<limit {
        return i
    }

    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_break_and_continue_inside_range_for() {
    let diagnostics = check_text(
        r#"func main(): void {
    for i in 0..<4 {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_range_for_non_integer_bound() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    for i in "a"..<4 {
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0360");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_range_for_bound_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let start: u16 = 0
    let end: u8 = 4

    for i in start..<end {
    }

    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0360");
    assert!(diagnostics[0].message.contains("u16"));
    assert!(diagnostics[0].message.contains("u8"));
}

#[test]
fn diagnoses_range_for_as_non_terminal_statement() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    for i in 0..<1 {
        return i
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_loop_with_return_as_terminal_statement() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    loop {
        return 0
    }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_terminal_loop_with_break() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    loop {
        break
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn diagnoses_break_outside_loop() {
    let diagnostics = check_text(
        r#"func main(): void {
    break
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0359");
    assert!(diagnostics[0].message.contains("break"));
}

#[test]
fn diagnoses_continue_outside_loop() {
    let diagnostics = check_text(
        r#"func main(): void {
    continue
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0359");
    assert!(diagnostics[0].message.contains("continue"));
}
