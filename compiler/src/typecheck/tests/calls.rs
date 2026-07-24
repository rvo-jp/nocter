use super::check_text;

#[test]
fn uses_same_file_function_call_return_type() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return title()
}

func title(): &str {
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
fn accepts_readonly_borrow_function_argument() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 1
    inspect(&value)
    return 0
}

func inspect(value: &i32): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_borrow_function_argument_from_var() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var value = 1
    touch(&+value)
    return 0
}

func touch(value: &+i32): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_implicit_non_copy_struct_argument() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 2 }
    return length(text)
}

func length(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0392");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("text"));
}

#[test]
fn diagnoses_implicit_non_copy_struct_field_argument() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

struct Wrap {
    text: Text
}

func main(): i32 {
    let wrap = Wrap{ text: Text{ len: 42 } }
    return length(wrap.text)
}

func length(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0392");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("wrap.text"));
}

#[test]
fn accepts_moved_non_copy_struct_argument() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
}

func main(): i32 {
    let text = Text{ start: 1, len: 2 }
    return length(move text)
}

func length(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_implicit_non_copy_generic_struct_argument_with_type_arguments() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32>{
        value: 1,
    }
    return value(box)
}

func value(box: Box<i32>): i32 {
    return box.value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0392");
    assert!(diagnostics[0].message.contains("Box<i32>"));
}

#[test]
fn accepts_associated_function_return_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

pub func Point.origin(): Self {
    return Self{ x: 0 }
}

func main(): i32 {
    return Point.origin().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_enum_associated_function_return_type() {
    let diagnostics = check_text(
        r#"enum Choice {
    empty
}

pub func Choice.answer(): i32 {
    return 42
}

func main(): i32 {
    return Choice.answer()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_unknown_associated_function_on_struct() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

func main(): i32 {
    return Parser.missing()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0388");
    assert!(diagnostics[0].message.contains("Parser"));
    assert!(diagnostics[0].message.contains("missing"));
}

#[test]
fn diagnoses_associated_function_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

pub func Parser.parse(value: i32): i32 {
    return value
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

pub func Parser.parse(value: i32): i32 {
    return value
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

pub func Parser.parse(): i32 {
    return "bad"
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

pub func Parser.parse(): i32 {
    return needs_value()
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

#[test]
fn infers_generic_function_return_type_from_argument() {
    let diagnostics = check_text(
        r#"func identity<T>(value: T): T {
    return value
}

func main(): i32 {
    let value: i32 = identity("hello")
    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn infers_generic_function_return_type_from_generic_struct_argument() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

func unwrap<T>(box: Box<T>): T {
    return box.value
}

func main(): i32 {
    let box = Box<i32>{
        value: 1,
    }
    return unwrap(move box)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn infers_generic_function_type_argument_from_binding_annotation() {
    let diagnostics = check_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}

func main(): i32 {
    let marker: Marker<u8> = make()
    return marker.code
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn infers_generic_function_type_argument_from_return_type() {
    let diagnostics = check_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T>{ code: 42 }
}

func make_u8(): Marker<u8> {
    return make()
}

func main(): i32 {
    return make_u8().code
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn checks_repeated_generic_function_parameter_types() {
    let diagnostics = check_text(
        r#"func same<T>(left: T, right: T): void {
}

func main(): i32 {
    same(1, "bad")
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0321");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn infers_generic_primitive_return_type_through_borrow_parameter() {
    let diagnostics = check_text(
        r#"primitive from_ref<T>(value: &T): *T

func main(): i32 {
    let value = 1
    let pointer: *i32 = from_ref(&value)
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_generic_primitive_return_type_mismatch_after_inference() {
    let diagnostics = check_text(
        r#"primitive from_ref<T>(value: &T): *T

func main(): i32 {
    let pointer: *i32 = from_ref("bad")
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("*i32"));
    assert!(diagnostics[0].message.contains("*str"));
}
