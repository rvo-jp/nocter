use super::check_text;

#[test]
fn accepts_struct_field_access() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    label: &str
}

func main(): i32 {
    return 0
}

func x(point: Point): i32 {
    return point.x
}

func label(point: Point): &str {
    return point.label
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_borrowed_struct_field_access() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    return 0
}

func x(point: &Point): i32 {
    return point.x
}

func set_x(point: &+Point): void {
    point.x = 2
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn uses_struct_field_type_for_return_checking() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    return 0
}

func x(point: Point): &str {
    return point.x
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_unknown_struct_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    return 0
}

func y(point: Point): i32 {
    return point.y
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("Point"));
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_field_access_on_non_struct_value() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func invalid(value: i32): i32 {
    return value.x
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0371");
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn accepts_struct_literal_expression() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    label: &str
}

func main(): i32 {
    let point = Point{
        label: "home",
        x: 1,
    }
    return point.x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_contextual_integer_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Byte {
    value: u8
}

func main(): i32 {
    let byte = Byte{
        value: 255,
    }
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_struct_literal_target() {
    let diagnostics = check_text(
        r#"type Number = i32

func main(): i32 {
    let value = Number{
        value: 1,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0372");
}

#[test]
fn diagnoses_unknown_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    let point = Point{
        x: 1,
        y: 2,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0373");
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_duplicate_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    let point = Point{
        x: 1,
        x: 2,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0374");
    assert!(diagnostics[0].message.contains("x"));
}

#[test]
fn diagnoses_missing_struct_literal_field() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
    y: i32
}

func main(): i32 {
    let point = Point{
        x: 1,
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0375");
    assert!(diagnostics[0].message.contains("y"));
}

#[test]
fn diagnoses_struct_literal_field_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    let point = Point{
        x: "bad",
    }
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0376");
    assert!(diagnostics[0].message.contains("str"));
    assert!(diagnostics[0].message.contains("i32"));
}
