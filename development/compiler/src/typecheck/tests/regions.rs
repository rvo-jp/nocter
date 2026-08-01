use super::check_text;

#[test]
fn diagnoses_direct_region_handle_return() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func leak(parent: Arena): Arena {
    region temp using parent {
        return temp
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
    assert!(diagnostics[0].message.contains("region `temp`"));
    assert_eq!(diagnostics[0].notes.len(), 1);
}

#[test]
fn diagnoses_region_handle_nested_in_owned_aggregate() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

copy struct Holder {
    arena: Arena
}

func leak(parent: Arena): Holder {
    region temp using parent {
        return Holder { arena: temp }
    }
}


func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0436");
}

#[test]
fn permits_region_independent_copy_result() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func value(parent: Arena): usize {
    region temp using parent {
        return 42
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
fn diagnoses_assignment_of_region_value_to_outer_binding() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func leak(parent: Arena): void {
    var escaped = parent
    region temp using parent {
        escaped = temp
    }
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0437");
    assert_eq!(diagnostics[0].notes.len(), 2);
}

#[test]
fn diagnoses_effectful_region_parent_expression() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func make_arena(): Arena {
    return Arena { id: 0 }
}

func use_region(): void {
    region temp using make_arena() {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0438");
}

#[test]
fn accepts_nested_regions_with_established_parents() {
    let diagnostics = check_text(
        r#"copy struct Arena {
    id: usize
}

func use_regions(parent: Arena): void {
    region outer using parent {
        region inner using outer {
            let value = 1
        }
    }
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
