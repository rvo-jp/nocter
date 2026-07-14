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
fn accepts_copy_struct_after_move_expression() {
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

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
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
