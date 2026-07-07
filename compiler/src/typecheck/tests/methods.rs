use super::check_text;

#[test]
fn accepts_method_body_receiver_self_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return point.x
    }
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_method_call_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return point.x
    }
}

program(): i32 {
    let point = Point{ x: 1 }
    return point.x_value()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_method_call_self_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).same(): Self {
        return Self{ x: point.x }
    }
}

program(): i32 {
    let point = Point{ x: 1 }
    return point.same().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_method_call_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    method (parser: Self).parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    let parser = Parser{ value: 0 }
    return parser.parse()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0320");
    assert!(diagnostics[0].message.contains("method `Parser.parse`"));
}

#[test]
fn diagnoses_method_call_argument_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    method (parser: Self).parse(value: i32): i32 {
        return value
    }
}

program(): i32 {
    let parser = Parser{ value: 0 }
    return parser.parse("bad")
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
}

#[test]
fn accepts_readwrite_method_call_on_var_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    var file = File{ fd: 1 }
    return file.write()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_readwrite_method_call_on_let_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    return file.write()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0378");
}

#[test]
fn diagnoses_readwrite_method_call_on_temporary() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &+Self).write(): i32 {
        return 0
    }
}

program(): i32 {
    return File{ fd: 1 }.write()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0378");
}

#[test]
fn accepts_readonly_method_call_on_let_binding_and_temporary() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: &Self).fd_value(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    if file.fd_value() == 0 {
        return File{ fd: 2 }.fd_value()
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unsupported_method_receiver_type() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    method (file: i32).bad(): i32 {
        return 0
    }
}

program(): i32 {
    let file = File{ fd: 1 }
    return file.bad()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0377");
}

#[test]
fn diagnoses_method_body_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method (point: Self).x_value(): i32 {
        return "bad"
    }
}

program(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("method `Point.x_value`"));
}
