use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_accepts_builtin_str_return_type() {
    let root = make_temp_project("builtin-str-return");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    return 0
}

func title(): &str {
    return "Nocter"
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_diagnoses_mismatched_builtin_str_return_type() {
    let root = make_temp_project("builtin-str-return-mismatch");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    return 0
}

func title(): &str {
    return 1
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn check_rejects_project_owned_builtin_implementation() {
    let root = make_temp_project("project-builtin-impl");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"impl str {
    pub method &self.project_method(): usize { return 0 }
}

func main(): i32 { return 0 }
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0416");
}

#[test]
fn check_rejects_malformed_standard_builtin_implementation() {
    let root = make_temp_project("malformed-standard-builtin-impl");
    let home = make_nocter_home(&root);
    fs::write(
        home.join("std/str.nct"),
        r#"impl<T> str {
    pub method self.consume(): usize { return 0 }
}
"#,
    )
    .unwrap();
    fs::write(root.join("app.nct"), "func main(): i32 { return 0 }\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0417"),
        "{diagnostics:?}"
    );
}
