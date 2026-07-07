use super::check_text;

#[test]
fn uses_same_file_function_call_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return title()
}

func title(): str {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_same_file_function_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return answer()
}

func answer(value: i32): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0320");
}

#[test]
fn diagnoses_same_file_function_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return length("hello")
}

func length(value: i32): i32 {
    return 1
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn accepts_associated_function_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    pub func origin(): Self {
        return Self{ x: 0 }
    }
}

func main(): i32 {
    return Point.origin().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_associated_function_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(value: i32): i32 {
        return value
    }
}

func main(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
    assert!(diagnostics[0].message.contains("Parser.parse"));
}

#[test]
fn diagnoses_associated_function_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(value: i32): i32 {
        return value
    }
}

func main(): i32 {
    return Parser.parse("bad")
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn diagnoses_associated_function_body_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(): i32 {
        return "bad"
    }
}

func main(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(
        diagnostics[0]
            .message
            .contains("associated function `Parser.parse`")
    );
}

#[test]
fn diagnoses_associated_function_body_call_argument_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    pub func parse(): i32 {
        return needs_value()
    }
}

func needs_value(value: i32): i32 {
    return value
}

func main(): i32 {
    return Parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
}
