use super::check_text;

const SURFACE: &str = r#"
struct Box<T> {
    pub value: T,
}

instance Box<T> {
    pub coerce &self as &T from self {
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
instance Box<T> {
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
instance First { pub coerce &self as &Second from self { return &self.value } }
instance Second { pub coerce &self as &i32 from self { return &self.value } }
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
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
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

#[test]
fn explicit_as_selects_a_generic_borrow_coercion() {
    let diagnostics = check_text(&format!(
        "{SURFACE}\nfunc project(value: &Box<i32>): &i32 from value {{\n    return value as &i32\n}}\n"
    ));

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn explicit_as_requires_the_source_borrow_to_be_written() {
    let diagnostics = check_text(&format!(
        "{SURFACE}\nfunc project(value: Box<i32>): &i32 {{\n    return value as &i32\n}}\n"
    ));

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an explicit source borrow")
    }));
}

#[test]
fn explicit_as_reports_insufficient_readwrite_capability() {
    let diagnostics = check_text(
        r#"struct Cell { value: i32 }
instance Cell { pub coerce &+self as &+i32 from self { return &+self.value } }
func project(value: &Cell): &+i32 from value { return value as &+i32 }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires a readwrite source borrow")
    }));
}

#[test]
fn explicit_as_supports_readwrite_to_readonly_capability_weakening() {
    let diagnostics = check_text(
        r#"struct Cell { value: i32 }
func project(value: &+Cell): &Cell from value { return value as &Cell }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn explicit_as_does_not_implicitly_chain_coercions() {
    let diagnostics = check_text(
        r#"struct First { value: Second }
struct Second { value: i32 }
instance First { pub coerce &self as &Second from self { return &self.value } }
instance Second { pub coerce &self as &i32 from self { return &self.value } }
func project(value: &First): &i32 from value { return value as &i32 }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("neither lossless integer conversion nor an accessible borrow coercion")
    }));
}

#[test]
fn explicit_as_does_not_accept_redundant_non_integer_conversions() {
    let diagnostics = check_text(
        r#"func identity(value: bool): bool { return value as bool }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("neither lossless integer conversion nor an accessible borrow coercion")
    }));
}

#[test]
fn contextual_coercion_applies_to_enum_payload_arguments() {
    let diagnostics = check_text(
        r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
enum View<T> { one(value: &T) }
func project(value: &Box<i32>): View<i32> from value { return View.one(value) }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn generic_coercion_requirement_supports_contextual_and_explicit_selection() {
    let diagnostics = check_text(
        r#"struct Text { value: &str }
instance Text { pub coerce &self as &str from self { return self.value } }
func contextual<T>(value: &T): &str from value where &T as &str { return value }
func explicit<T>(value: &T): &str from value where &T as &str { return value as &str }
func demo(value: &Text): &str from value { return contextual(value) }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn generic_coercion_requirement_supports_receiver_method_selection() {
    let diagnostics = check_text(
        r#"struct View {}
instance View { pub method &self.len(): usize { return 0 } }
struct Text { view: View }
instance Text { pub coerce &self as &View from self { return &self.view } }
func len<T>(value: &T): usize where &T as &View { return value.len() }
func demo(value: &Text): usize { return len(value) }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn generic_coercion_requirement_supports_comparison_and_index_selection() {
    let diagnostics = check_text(
        r#"struct View { value: i32 }
instance View {
    pub operator (&self == other: &Self): bool { return self.value == other.value }
}
struct Text { view: View }
instance Text { pub coerce &self as &View from self { return &self.view } }
struct Buffer { values: &[u8] }
instance Buffer { pub coerce &self as &[u8] from self { return self.values } }
func same<T>(left: &T, right: &T): bool where &T as &View { return left == right }
func first<T>(value: &T): u8 where &T as &[u8] { return value[0] }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn generic_coercion_requirement_is_checked_at_concrete_calls() {
    let diagnostics = check_text(
        r#"struct Other {}
func view<T>(value: &T): &str from value where &T as &str { return value }
func demo(value: &Other): &str from value { return view(value) }
func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0479"
            && diagnostic
                .message
                .contains("coercion requirement `&Other as &str` is not satisfied")
    }));
}
