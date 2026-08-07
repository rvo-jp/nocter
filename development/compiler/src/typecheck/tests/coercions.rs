use super::check_text;

const SURFACE: &str = r#"
struct Box<T> {
    pub value: T,
}

coerce Box<T> {
    pub &self as &T from self {
        return &self.value
    }
}

func accept(value: &i32): void {
    return
}

func main(): i32 {
    return 0
}
"#;

#[test]
fn borrowed_nominal_value_coerces_at_a_concrete_argument_boundary() {
    let diagnostics = check_text(&format!(
        "{SURFACE}\nfunc demo(value: &Box<i32>): void {{\n    accept(value)\n    return\n}}\n"
    ));
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn coercion_body_has_the_same_receiver_provenance_as_an_inherent_method() {
    let diagnostics = check_text(
        r#"
struct Box<T> { value: T }
impl<T> Box<T> {
    pub method &self.get(): &T from self { return &self.value }
}
func main(): i32 { return 0 }
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn owned_value_is_not_implicitly_borrowed_for_coercion() {
    let diagnostics = check_text(&format!(
        "{SURFACE}\nfunc demo(value: Box<i32>): void {{\n    accept(value)\n    return\n}}\n"
    ));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("argument 1"))
    );
}

#[test]
fn readwrite_borrow_may_use_a_readonly_coercion() {
    let diagnostics = check_text(&format!(
        "{SURFACE}\nfunc demo(value: &+Box<i32>): void {{\n    accept(value)\n    return\n}}\n"
    ));
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn coercions_do_not_chain() {
    let diagnostics = check_text(
        r#"
struct First { pub value: Second }
struct Second { pub value: i32 }
coerce First { pub &self as &Second from self { return &self.value } }
coerce Second { pub &self as &i32 from self { return &self.value } }
func accept(value: &i32): void { return }
func demo(value: &First): void { accept(value) return }
func main(): i32 { return 0 }
"#,
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("argument 1"))
    );
}

#[test]
fn coercions_apply_at_every_concrete_expected_type_boundary() {
    let diagnostics = check_text(
        r#"
struct Box<T> { value: T }
coerce Box<T> { pub &self as &T from self { return &self.value } }
struct Holder { value: &i32 }
func accept(value: &i32): void { return }
func project(value: &Box<i32>): &i32 from value {
    let bound: &i32 = value
    var assigned: &i32 = bound
    assigned = value
    let holder = Holder { value: value }
    let elements: [&i32; 1] = [value]
    accept(value)
    return value
}
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}
