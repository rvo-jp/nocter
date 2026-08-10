use super::check_text;

#[test]
fn accepts_destruct_readwrite_self_borrow_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
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
fn diagnoses_destruct_on_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

destruct Pair(&+self) {
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0391");
    assert!(diagnostics[0].message.contains("Pair"));
}

#[test]
fn rejects_repeated_destruct_pattern_binders_but_accepts_uniform_generic_destruct() {
    let repeated = check_text(
        r#"struct Pair<L, R> { left: L, right: R }
destruct Pair<T, T>(&+self) { return }
func main(): i32 { return 0 }
"#,
    );
    assert!(
        repeated.iter().any(|diagnostic| diagnostic.code == "E0388"),
        "{repeated:?}"
    );

    let uniform = check_text(
        r#"struct Box<T> { value: T }
destruct Box<T>(&+self) { return }
func main(): i32 { return 0 }
"#,
    );
    assert!(uniform.is_empty(), "{uniform:?}");
}

#[test]
fn rejects_alias_and_view_destruct_targets() {
    for source in [
        "type Handle = i32\ndestruct Handle(&+self) { return }\nfunc main(): i32 { return 0 }\n",
        "destruct [T](&+self) { return }\nfunc main(): i32 { return 0 }\n",
    ] {
        let diagnostics = check_text(source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E0399"),
            "{source}: {diagnostics:?}"
        );
    }
}
