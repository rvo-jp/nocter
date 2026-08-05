use super::check_text;

#[test]
fn infers_sequence_element_type_and_accepts_explicit_empty_target() {
    let diagnostics = check_text(
        r#"struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self { return Self { marker: 0 } }
}

func inferred(): Vec<i32> {
    return Vec [1, 2, 3]
}

func empty(): Vec<i32> {
    return Vec<i32> []
}

func contextual_empty(): Vec<i32> {
    return Vec []
}

func main(): void {}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn rejects_incompatible_sequence_elements_and_unconstrained_empty_literal() {
    let mismatch = check_text(
        r#"struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self { return Self { marker: 0 } }
}

func build(): Vec<i32> {
    return Vec [1, "wrong"]
}

func main(): void {}
"#,
    );
    assert!(mismatch.iter().any(|diagnostic| diagnostic.code == "E0521"));

    let empty = check_text(
        r#"struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self { return Self { marker: 0 } }
}

func main(): void {
    let unconstrained = Vec []
}
"#,
    );
    assert!(empty.iter().any(|diagnostic| diagnostic.code == "E0522"));
}

#[test]
fn restricts_string_parameter_and_literal_pack_operations() {
    let diagnostics = check_text(
        r#"struct Text {}
construct Text {
    pub default literal ""(text: i32): Self {}
}

struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self {
        let escaped = items
        return Self { marker: 0 }
    }
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0520")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0526")
    );
}

#[test]
fn accepts_pack_len_and_one_consuming_loop() {
    let diagnostics = check_text(
        r#"struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self {
        let count: usize = items.len()
        for item in items {
            consume(move item)
        }
        return Self { marker: 0 }
    }
}

func consume<T>(value: T): void {}
func main(): void {}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn rejects_loop_control_that_would_partially_consume_a_phase_one_pack() {
    let diagnostics = check_text(
        r#"struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self {
        for item in items {
            break
        }
        return Self { marker: 0 }
    }
}

func main(): void {}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0528")
    );
}
