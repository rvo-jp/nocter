use super::check_text;

#[test]
fn accepts_method_body_receiver_self_type() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method self.x_value(): i32 {
        return self.x
    }
}

func main(): i32 {
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
    method self.x_value(): i32 {
        return self.x
    }
}

func main(): i32 {
    let point = Point { x: 1 }
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
    method self.same(): Self {
        return Self { x: self.x }
    }
}

func main(): i32 {
    let point = Point { x: 1 }
    return point.same().x
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readonly_borrow_method_receiver() {
    let diagnostics = check_text(
        r#"struct Counter {
    value: i32
}

impl Counter {
    method &self.get(): i32 {
        return self.value
    }
}

func inspect(counter: &Counter): i32 {
    return counter.get()
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_borrow_method_receiver() {
    let diagnostics = check_text(
        r#"struct Counter {
    value: i32
}

impl Counter {
    method &+self.bump(): void {
        self.value = self.value + 1
        return
    }
}

func bump(counter: &+Counter): void {
    counter.bump()
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_generic_impl_method_body_and_call_return_type() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> {
        value: 1,
    }
    return box.value()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_method_local_generic_inferred_from_an_argument() {
    let diagnostics = check_text(
        r#"struct Factory {
    marker: i32
}

impl Factory {
    method &self.identity<T>(value: T): T {
        return value
    }
}

func main(): i32 {
    let factory = Factory { marker: 0 }
    return factory.identity(7)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_method_generic_reusing_an_impl_parameter() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

impl<T> Box<T> {
    method &self.identity<T>(value: T): T {
        return value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0420"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_generic_impl_method_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.bad(): i32 {
        return self.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("U"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_method_call_from_non_matching_generic_impl_target() {
    let diagnostics = check_text(
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method self.value_i32(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<&str> {
        value: "bad",
    }
    return box.value_i32()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0389");
    assert!(diagnostics[0].message.contains("Box<&str>"));
    assert!(diagnostics[0].message.contains("value_i32"));
}

#[test]
fn diagnoses_unknown_method_call_on_struct_value() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

func main(): i32 {
    let parser = Parser { value: 0 }
    return parser.missing()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0389");
    assert!(diagnostics[0].message.contains("Parser"));
    assert!(diagnostics[0].message.contains("missing"));
}

#[test]
fn diagnoses_field_call_as_method() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

func main(): i32 {
    let point = Point { x: 0 }
    return point.x()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0389");
    assert!(diagnostics[0].message.contains("field `Point.x`"));
}

#[test]
fn diagnoses_method_call_argument_count_mismatch() {
    let diagnostics = check_text(
        r#"struct Parser {
    value: i32
}

impl Parser {
    method self.parse(value: i32): i32 {
        return value
    }
}

func main(): i32 {
    let parser = Parser { value: 0 }
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
    method self.parse(value: i32): i32 {
        return value
    }
}

func main(): i32 {
    let parser = Parser { value: 0 }
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
    method &+self.write(): i32 {
        return 0
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    return file.write()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_method_call_on_var_aggregate_field() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

struct Holder {
    file: File
}

impl File {
    method &+self.write(): i32 {
        return 0
    }
}

func main(): i32 {
    var holder = Holder { file: File { fd: 1 } }
    return holder.file.write()
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
    method &+self.write(): i32 {
        return 0
    }
}

func main(): i32 {
    let file = File { fd: 1 }
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
    method &+self.write(): i32 {
        return 0
    }
}

func main(): i32 {
    return File { fd: 1 }.write()
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
    method &self.fd_value(): i32 {
        return 0
    }
}

func main(): i32 {
    let file = File { fd: 1 }
    if file.fd_value() == 0 {
        return File { fd: 2 }.fd_value()
    }
    return 1
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_method_body_return_type_mismatch() {
    let diagnostics = check_text(
        r#"struct Point {
    x: i32
}

impl Point {
    method self.x_value(): i32 {
        return "bad"
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("method `Point.x_value`"));
}
