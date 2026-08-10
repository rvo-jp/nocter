use super::check_text;

#[test]
fn accepts_drop_member_readwrite_self_borrow_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

instance File {
    drop &+self {
        return
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
fn diagnoses_drop_member_on_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

instance Pair {
    drop &+self {
        return
    }
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
fn rejects_conditional_drop_patterns_but_accepts_uniform_generic_drop() {
    let conditional = check_text(
        r#"struct Box<T> { value: T }
instance Box<T> where T = i32 {
    drop &+self { return }
}
func main(): i32 { return 0 }
"#,
    );
    assert!(
        conditional
            .iter()
            .any(|diagnostic| diagnostic.code == "E0388"),
        "{conditional:?}"
    );

    let uniform = check_text(
        r#"struct Box<T> { value: T }
instance Box<T> {
    drop &+self { return }
}
func main(): i32 { return 0 }
"#,
    );
    assert!(uniform.is_empty(), "{uniform:?}");
}
