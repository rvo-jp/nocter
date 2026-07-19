use super::check_text;

#[test]
fn diagnoses_binding_annotation_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let byte: u8 = 300
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0342");
    assert!(diagnostics[0].message.contains("u8"));
    assert!(diagnostics[0].message.contains("i32"));
}

#[test]
fn diagnoses_assignment_to_let_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let count = 0
    count = 1
    return count
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("count"));
}

#[test]
fn diagnoses_member_assignment_to_let_binding() {
    let diagnostics = check_text(
        r#"struct Header {
    code: i32
}

func main(): i32 {
    let value = Header{ code: 1 }
    value.code = 2
    return value.code
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_member_assignment_through_readonly_borrow() {
    let diagnostics = check_text(
        r#"struct Header {
    code: i32
}

func main(): i32 {
    return 0
}

func update(value: &Header): void {
    value.code = 2
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_member_assignment_through_readonly_borrow_var_binding() {
    let diagnostics = check_text(
        r#"struct Header {
    code: i32
}

func main(): i32 {
    return 0
}

func update(source: &Header): void {
    var value = source
    value.code = 2
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_assignment_to_parameter_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    return id(0)
}

func id(value: i32): i32 {
    value = 1
    return value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0381");
    assert!(diagnostics[0].message.contains("value"));
}

#[test]
fn diagnoses_readwrite_borrow_of_let_binding() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    let value = 1
    touch(&+value)
    return 0
}

func touch(value: &+i32): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0383");
}

#[test]
fn diagnoses_readwrite_borrow_of_readonly_borrow_field_var_binding() {
    let diagnostics = check_text(
        r#"struct Header {
    code: i32
}

func main(): i32 {
    return 0
}

func update(source: &Header): void {
    var value = source
    touch(&+value.code)
    return
}

func touch(value: &+i32): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0383");
}

#[test]
fn diagnoses_assignment_type_mismatch() {
    let diagnostics = check_text(
        r#"func main(): i32 {
    var count: i32 = 0
    count = "hello"
    return count
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0382");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn diagnoses_assignment_from_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var source = Text{ start: 1, len: 2, capacity: 3 }
    var target = Text{ start: 4, len: 5, capacity: 6 }
    target = source
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0384");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("source"));
}

#[test]
fn diagnoses_binding_from_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let source = Text{ start: 1, len: 2, capacity: 3 }
    let target = source
    return target.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0432");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("source"));
    assert!(diagnostics[0].message.contains("target"));
}

#[test]
fn diagnoses_binding_from_non_copy_struct_field() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

struct Wrap {
    text: Text
}

func main(): i32 {
    let wrap = Wrap{ text: Text{ len: 42 } }
    let text = wrap.text
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0432");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("wrap.text"));
}

#[test]
fn accepts_binding_from_moved_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let source = Text{ start: 1, len: 2, capacity: 3 }
    let target = move source
    return target.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_optional_binding_from_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    let source = Text{ start: 1, len: 2, capacity: 3 }
    let target: Text? = source
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0432");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("source"));
    assert!(diagnostics[0].message.contains("target"));
}

#[test]
fn accepts_assignment_from_moved_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var source = Text{ start: 1, len: 2, capacity: 3 }
    var target = Text{ start: 4, len: 5, capacity: 6 }
    target = move source
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_assignment_from_non_copy_struct_field() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

struct Wrap {
    text: Text
}

func main(): i32 {
    var text = Text{ len: 1 }
    let wrap = Wrap{ text: Text{ len: 42 } }
    text = wrap.text
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0384");
    assert!(diagnostics[0].message.contains("Text"));
    assert!(diagnostics[0].message.contains("wrap.text"));
}

#[test]
fn diagnoses_self_move_assignment_from_non_copy_struct_binding() {
    let diagnostics = check_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var target = Text{ start: 1, len: 2, capacity: 3 }
    target = move target
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0395");
    assert!(diagnostics[0].message.contains("target"));
}

#[test]
fn accepts_assignment_from_copy_struct_binding() {
    let diagnostics = check_text(
        r#"copy struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    var source = Text{ start: 1, len: 2, capacity: 3 }
    var target = Text{ start: 4, len: 5, capacity: 6 }
    target = source
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
