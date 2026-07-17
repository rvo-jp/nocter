use super::check_text;

#[test]
fn accepts_drop_member_readwrite_self_borrow_binding() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
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
fn diagnoses_drop_member_value_self_binding_type() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: Self {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0387");
}

#[test]
fn diagnoses_drop_member_readonly_self_borrow_binding_type() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &Self {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0387");
}

#[test]
fn diagnoses_drop_member_on_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

impl Pair {
    drop pair: &+Self {
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
