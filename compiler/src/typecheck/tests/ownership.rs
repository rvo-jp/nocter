use super::check_text;

#[test]
fn diagnoses_use_after_move_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    let length = take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_double_move_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    let first = take(move text)
    let second = take(move text)
    return first + second
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn invalid_outer_move_operand_does_not_consume_nested_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    let invalid = move take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("binding"));
}

#[test]
fn diagnoses_use_after_explicit_drop_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    drop text
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("dropped"));
}

#[test]
fn diagnoses_double_explicit_drop_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    drop text
    drop text
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("drop"));
    assert!(diagnostics[0].message.contains("dropped"));
}

#[test]
fn diagnoses_explicit_drop_of_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let pair = Pair{ left: 1, right: 2 }
    drop pair
    return pair.left + pair.right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0386");
    assert!(diagnostics[0].message.contains("Pair"));
}

#[test]
fn diagnoses_move_of_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let pair = Pair{ left: 20, right: 22 }
    let copied = move pair
    return pair.left + copied.right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0394");
    assert!(diagnostics[0].message.contains("Pair"));
}

#[test]
fn accepts_var_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text{ start: 1, len: 20, capacity: 3 }
    let first = take(move text)
    text = Text{ start: 4, len: 22, capacity: 6 }
    return first + text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_use_after_owned_method_receiver_move() {
    let diagnostics = check_text(
        r#"struct Holder {
    value: i32
}

impl Holder {
    method (holder: Self).take(): i32 {
        return holder.value
    }
}

func main(): i32 {
    let holder = Holder{ value: 21 }
    let value = holder.take()
    return value + holder.value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("holder"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_one_if_branch_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    if true {
        let length = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_uninitialized_after_both_if_branches_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    if true {
        let first = take(move text)
    } else {
        let second = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_uninitialized_after_if_branches_move_and_drop() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    if true {
        let length = take(move text)
    } else {
        drop text
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn accepts_if_branch_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text{ start: 1, len: 20, capacity: 3 }
    if true {
        let first = take(move text)
        text = Text{ start: 4, len: 22, capacity: 6 }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_uninitialized_after_match_branches_move_and_drop() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    drop_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text{ start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }

        else {
            drop text
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn diagnoses_uninitialized_after_exhaustive_match_without_else_branches_move_and_drop() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    drop_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text{ start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }

        Choice.drop_it {
            drop text
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_match_without_else_moves() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    keep_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text{ start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_pattern_conditional_arm_moves() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    keep_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text{ start: 1, len: 42, capacity: 3 }
    let value = choice ?{
        Choice.move_it : take(move text)
        : 0
    }
    return text.len + value
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_catch_fallthrough_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    let value = fallible() catch error {
        let moved = take(move text)
    }
    return text.len + value
}

func fallible(): i32! {
    return 1
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn accepts_unreachable_use_after_returning_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    return take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_branch_return_after_move_without_poisoning_fallthrough() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    if true {
        return take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_maybe_uninitialized_after_while_body_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    while true {
        let length = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn accepts_while_body_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text{ start: 1, len: 20, capacity: 3 }
    while true {
        let length = take(move text)
        text = Text{ start: 4, len: 22, capacity: 6 }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_uninitialized_after_loop_break_drops() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 42, capacity: 3 }
    loop {
        drop text
        break
    }
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("dropped"));
}
