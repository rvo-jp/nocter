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
