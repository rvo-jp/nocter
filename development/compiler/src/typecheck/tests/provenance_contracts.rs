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
fn diagnoses_ambiguous_bodyless_interface_result() {
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
            .any(|diagnostic| diagnostic.code == "E0444")
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
fn accepts_alloc_contract_for_pointer_backed_owned_result() {
    let diagnostics = check_text(
        r#"alloc primitive allocate(): *u8

struct Buffer { ptr: *u8 }

alloc func build(): Buffer {
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
