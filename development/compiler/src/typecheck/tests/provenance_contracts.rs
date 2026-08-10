use super::check_text;

#[test]
fn accepts_result_provenance_contract_satisfied_by_body() {
    let diagnostics = check_text(
        r#"func choose(left: &i32, right: &i32): &i32 from left | static {
    return left
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_body_result_outside_declared_provenance() {
    let diagnostics = check_text(
        r#"func choose(left: &i32, right: &i32): &i32 from left {
    return right
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0445")
    );
}

#[test]
fn diagnoses_invalid_and_duplicate_result_origins() {
    let diagnostics = check_text(
        r#"func bad(value: i32): &i32 from self | value | value {
    return &value
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0440")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0441")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0442")
    );
}

#[test]
fn diagnoses_ambiguous_bodyless_interface_without_from() {
    let diagnostics = check_text(
        r#"interface Choose {
    pub method &self.choose(other: &Self): &Self
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0446"),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_explicit_bodyless_interface_result_contract() {
    let diagnostics = check_text(
        r#"interface Choose {
    pub method &self.choose(other: &Self): &Self from self | other
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_bodyless_result_without_an_external_origin() {
    let diagnostics = check_text(
        r#"interface View {
    pub method &self.view(): &Self
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_elided_public_body_with_one_result_origin() {
    let diagnostics = check_text(
        r#"pub func view(value: &i32): &i32 { return value }

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_elided_public_body_with_multiple_possible_origins() {
    let diagnostics = check_text(
        r#"pub func choose(left: &i32, right: &i32): &i32 { return left }

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0444"),
        "{diagnostics:?}"
    );
}

#[test]
fn private_body_may_keep_an_inferred_external_result_origin() {
    let diagnostics = check_text(
        r#"func view(value: &i32): &i32 { return value }

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_public_inherent_method_with_elided_receiver_origin() {
    let diagnostics = check_text(
        r#"struct Holder { value: &i32 }

instance Holder {
    pub method &self.view(): &i32 { return self.value }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_public_fallible_method_with_elided_receiver_origin() {
    let diagnostics = check_text(
        r#"struct Holder { value: &i32 }

instance Holder {
    pub method &self.view(): &i32! { return self.value }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn private_inherent_method_may_infer_an_external_result_origin() {
    let diagnostics = check_text(
        r#"struct Holder { value: &i32 }

instance Holder {
    method &self.view(): &i32 { return self.value }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_public_literal_with_one_elided_parameter_origin() {
    let diagnostics = check_text(
        r#"struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self {
        return Text { value: text }
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_sequence_literal_capture_as_an_external_result_origin() {
    let diagnostics = check_text(
        r#"struct Holder<T> { value: T }
primitive stop(): never

construct Holder<T> {
    pub default literal [](...items: T): Self from items {
        for item in items {
            return Holder<T> { value: move item }
        }
        return stop()
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_sequence_literal_with_elided_element_pack_origin() {
    let diagnostics = check_text(
        r#"struct Holder<T> { value: T }
primitive stop(): never

construct Holder<T> {
    pub default literal [](...items: T): Self {
        for item in items {
            return Holder<T> { value: move item }
        }
        return stop()
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_ambiguous_callable_type_without_from() {
    let diagnostics = check_text(
        r#"func apply(
    callback: &func(left: &i32, right: &i32): &i32,
    left: &i32,
    right: &i32,
): &i32 from left | right {
    return callback(left, right)
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0446"),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_callable_type_with_one_elided_origin() {
    let diagnostics = check_text(
        r#"func apply<F>(callback: F, value: &i32): &i32 from value where F: &func(value: &i32): &i32 {
    return callback(value)
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_contract_on_storage_independent_result() {
    let diagnostics = check_text(
        r#"func size(): i32 from static {
    return 1
}

func main(): i32 {
    return size()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0443")
    );
}

#[test]
fn accepts_literal_result_provenance_contract_satisfied_by_body() {
    let diagnostics = check_text(
        r#"struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self from text {
        return Text { value: text }
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
fn diagnoses_literal_body_result_outside_declared_provenance() {
    let diagnostics = check_text(
        r#"struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self from static {
        return Text { value: text }
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
            .any(|diagnostic| diagnostic.code == "E0445")
    );
}

#[test]
fn infers_fresh_storage_for_pointer_backed_owned_result() {
    let diagnostics = check_text(
        r#"primitive allocate(): *u8

struct Buffer { ptr: *u8 }

func build(): Buffer {
    return Buffer { ptr: allocate() }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn internal_fresh_storage_and_external_provenance_are_independent() {
    let diagnostics = check_text(
        r#"primitive fresh(): &i32

func choose(existing: &i32, create: bool): &i32 from existing {
    if create {
        return fresh()
    }
    return existing
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E0445"),
        "{diagnostics:?}"
    );
}

#[test]
fn result_contract_ignores_scalar_fields_in_storage_carrying_aggregates() {
    let diagnostics = check_text(
        r#"struct Allocator { state: usize }
struct Buffer { ptr: *u8, len: usize }

primitive allocate(allocator: &+Allocator): *u8 from allocator

func make(allocator: &+Allocator, len: usize): Buffer from allocator {
    return Buffer { ptr: allocate(allocator), len: len }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E0445"),
        "{diagnostics:?}"
    );
}

#[test]
fn preserves_result_provenance_through_stored_outcome_bindings() {
    let diagnostics = check_text(
        r#"func forward(value: &i32?): &i32? from value {
    let saved = value
    return saved
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn forwarded_owned_inputs_keep_their_upstream_provenance() {
    let diagnostics = check_text(
        r#"struct BorrowingValue {
    value: &i32
}

func forward(value: BorrowingValue): BorrowingValue from value {
    var source = move value
    return move source
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
