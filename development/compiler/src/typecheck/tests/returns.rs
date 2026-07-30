use super::check_text;

#[test]
fn diagnoses_string_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return "hello"
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn diagnoses_bare_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0310");
}

#[test]
fn diagnoses_value_return_from_void_function() {
    let diagnostics = check_text(
        r#"func main(): void {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0311");
}

#[test]
fn diagnoses_missing_return_from_i32_function() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0313");
}

#[test]
fn accepts_unreachable_statement_tail_after_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
    let value = "unreachable"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_unreachable_body_result_after_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
    "unreachable"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_terminal_never_expression_statement() {
    let diagnostics = check_text(
        r#"primitive trap(): never

func main(): i32 {
    trap()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_from_never_function() {
    let diagnostics = check_text(
        r#"primitive trap(): never

func main(): i32 {
    return 0
}

func stop(): never {
    return trap()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0314");
}

#[test]
fn accepts_str_function_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func title(): &str {
    return "hello"
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_borrow_parameter_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(value: &i32): &i32 {
    return value
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(): &i32 {
    let value = 1
    return &value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_readwrite_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(): &+i32 {
    var value = 1
    return &+value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_borrow_of_owned_parameter() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(value: i32): &i32 {
    return &value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("parameter"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_borrow_of_temporary() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value(): i32 {
    return 1
}

func leak(): &i32 {
    return &value()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("temporary"));
}

#[test]
fn diagnoses_return_borrow_alias_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(): &i32 {
    let value = 1
    let view = &value
    return view
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_borrow_alias_chain_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(): &i32 {
    let value = 1
    let first = &value
    let second = first
    return second
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn accepts_borrow_parameter_alias_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(value: &i32): &i32 {
    let view = value
    return view
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_borrow_alias_after_assignment_from_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(value: &i32): &i32 {
    var view = value
    let local = 1
    view = &local
    return view
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn accepts_borrow_alias_reassigned_to_parameter_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(value: &i32): &i32 {
    let local = 1
    var view = &local
    view = value
    return view
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_borrow_alias_after_if_branch_assignment_from_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(value: &i32, condition: bool): &i32 {
    var view = value
    if condition {
        let local = 1
        view = &local
    }
    return view
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn diagnoses_body_result_if_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func leak(condition: bool): &i32 {
    let local = 1
    if condition {
        &local
    } else {
        &local
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn diagnoses_body_result_match_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

enum Choice {
    left
    right
}

func leak(choice: Choice): &i32 {
    let local = 1
    match choice {
        Choice.left {
            &local
        }
        _ {
            &local
        }
    }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn accepts_borrow_alias_after_all_if_branches_reassign_to_parameter() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(value: &i32, condition: bool): &i32 {
    let local = 1
    var view = &local
    if condition {
        view = value
    } else {
        view = value
    }
    return view
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_borrow_alias_when_escaping_assignment_branch_returns() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func same(value: &i32, condition: bool): &i32 {
    var view = value
    if condition {
        let local = 1
        view = &local
        return value
    }
    return view
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_borrow_like_call_from_local_borrow() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func identity(value: &i32): &i32 {
    return value
}

func leak(): &i32 {
    let local = 1
    return identity(&local)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn accepts_return_borrow_like_call_from_parameter_borrow() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func identity(value: &i32): &i32 {
    return value
}

func same(value: &i32): &i32 {
    return identity(value)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_return_static_borrow_like_call_with_local_borrow_argument() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func label(value: &i32): &str {
    return "static"
}

func ok(): &str {
    let local = 1
    return label(&local)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_borrow_like_call_alias_from_local_borrow() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func identity(value: &i32): &i32 {
    return value
}

func leak(): &i32 {
    let local = 1
    let view = identity(&local)
    return view
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("local"));
}

#[test]
fn diagnoses_return_borrow_like_method_receiver_from_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

struct Text {
    len: i32
}

impl Text {
    method &self.self_ref(): &Self {
        return self
    }
}

func leak(): &Text {
    let text = Text{ len: 42 }
    return text.self_ref()
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("text"));
}

#[test]
fn accepts_str_literal_alias_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func title(): &str {
    let text: &str = "hello"
    return text
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_struct_literal_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct View {
    value: &i32
}

func leak(): View {
    let value = 1
    return View{ value: &value }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_struct_literal_after_static_field_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct View {
    label: &str
    value: &i32
}

func leak(): View {
    let value = 1
    return View{ label: "static", value: &value }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_error_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"primitive make_error(value: &i32): error

func main(): i32 {
    return 0
}

func leak(): error {
    let value = 1
    let failure = make_error(&value)
    return failure
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_error_after_static_argument_and_local_borrow_argument() {
    let diagnostics = check_text(
        r#"primitive make_error(label: &str, value: &i32): error

func main(): i32 {
    return 0
}

func leak(): error {
    let value = 1
    return make_error("static", &value)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn accepts_return_error_borrow_of_parameter() {
    let diagnostics = check_text(
        r#"primitive make_error(value: &i32): error

func main(): i32 {
    return 0
}

func forward(value: &i32): error {
    let failure = make_error(value)
    return failure
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_return_struct_alias_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct View {
    value: &i32
}

func leak(): View {
    let value = 1
    let view = View{ value: &value }
    return view
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_nested_struct_literal_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct Inner {
    value: &i32
}

copy struct Outer {
    inner: Inner
}

func leak(): Outer {
    let value = 1
    return Outer{ inner: Inner{ value: &value } }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_generic_struct_literal_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

struct Box<T> {
    value: T
}

func leak(): Box<&i32> {
    let value = 1
    return Box<&i32>{ value: &value }
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_return_enum_payload_borrow_of_local_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

enum MaybeRef {
    some(value: &i32)
    empty
}

func leak(): MaybeRef {
    let value = 1
    return MaybeRef.some(&value)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0433");
    assert!(diagnostics[0].message.contains("local binding"));
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn accepts_return_struct_literal_borrow_of_parameter() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct View {
    value: &i32
}

func wrap(value: &i32): View {
    return View{ value: value }
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_return_struct_alias_borrow_of_parameter() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

copy struct View {
    value: &i32
}

func wrap(value: &i32): View {
    let view = View{ value: value }
    return view
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_bool_function_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    return true
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_generic_parameter_return_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value<T>(input: T): i32 {
    return input
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("T"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_generic_borrow_return_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return 0
}

func value<T>(input: &T): T {
    return input
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("&T"));
    assert!(diagnostics[0].message.contains("T"));
}

#[test]
fn diagnoses_implicit_non_copy_struct_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return make().len
}

struct Text {
    start: i32
    len: i32
}

func make(): Text {
    let text = Text{ start: 1, len: 42 }
    return text
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0393");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("text"));
}

#[test]
fn diagnoses_implicit_copy_struct_instantiation_return_with_non_copy_type_argument() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return make().value.len
}

struct Text {
    len: i32
}

copy struct Box<T> {
    value: T
}

func make(): Box<Text> {
    let box = Box<Text>{ value: Text{ len: 42 } }
    return box
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0393");
    assert!(
        diagnostics[0]
            .message
            .contains("move-only copy-struct instantiation")
    );
    assert!(diagnostics[0].message.contains("Box<Text>"));
    assert!(diagnostics[0].message.contains("box"));
}

#[test]
fn diagnoses_implicit_move_only_fixed_array_return() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    return 0
}

func make(values: [Text; 1]): [Text; 1] {
    return values
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0393");
    assert!(diagnostics[0].message.contains("move-only fixed array"));
    assert!(diagnostics[0].message.contains("[Text; 1]"));
    assert!(diagnostics[0].message.contains("values"));
}

#[test]
fn diagnoses_implicit_non_copy_struct_field_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return extract().len
}

struct Text {
    len: i32
}

struct Wrap {
    text: Text
}

func extract(): Text {
    let wrap = Wrap{ text: Text{ len: 42 } }
    return wrap.text
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0393");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("wrap.text"));
}

#[test]
fn accepts_moved_non_copy_struct_return() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return make().len
}

struct Text {
    start: i32
    len: i32
}

func make(): Text {
    let text = Text{ start: 1, len: 42 }
    return move text
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
