use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_accepts_builtin_str_return_type() {
    let root = make_temp_project("builtin-str-return");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
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
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_diagnoses_mismatched_builtin_str_return_type() {
    let root = make_temp_project("builtin-str-return-mismatch");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
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
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("&str"));
}

#[test]
fn check_rejects_project_owned_builtin_instance() {
    let root = make_temp_project("project-builtin-impl");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"instance str {
    pub method &self.project_method(): usize { return 0 }
}

func main(): i32 { return 0 }
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0416");
}

#[test]
fn check_rejects_project_owned_builtin_construction() {
    let root = make_temp_project("project-builtin-construction");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"construct error {
    pub default func new(code: &str, message: &str): Self {
        return error.new(code, message)
    }
}

func main(): i32 { return 0 }
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0416"
                && diagnostic
                    .message
                    .contains("construction for built-in type `error`")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn check_resolves_authorized_builtin_error_construction() {
    let root = make_temp_project("builtin-error-construction");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        home.join("std/error/index.nct"),
        r#"pub(/) primitive new_error(code: &str, message: &str): error

construct error {
    pub default func new(code: &str, message: &str): Self from code | message {
        return new_error(code, message)
    }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        r#"func fail(): i32! {
    return error.new("app.failed", "failed")
}

func main(): i32 {
    return fail() catch _ { return 7 }
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_rejects_builtin_construction_that_does_not_produce_self() {
    let root = make_temp_project("invalid-builtin-construction-result");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        home.join("std/error/index.nct"),
        r#"construct error {
    pub default func new(code: &str, message: &str): usize {
        return 0
    }
}
"#,
    )
    .unwrap();
    crate::test_files::write(root.join("index.nct"), "func main(): i32 { return 0 }\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0417" && diagnostic.message.contains("must produce `Self`")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn check_accepts_scalar_instance_from_shared_num_authority() {
    let root = make_temp_project("builtin-scalar-shared-authority");
    let home = make_nocter_home(&root);
    fs::create_dir_all(home.join("std/num")).unwrap();
    crate::test_files::write(
        home.join("std/num/index.nct"),
        r#"instance bool {
    pub method &self.as_usize(): usize {
        return 1
    }
}

pub func load(): void { return }
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        r#"use std/num.load

func inspect(): usize { return true.as_usize() }
func main(): i32 { return 0 }
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_rejects_malformed_standard_builtin_instance() {
    let root = make_temp_project("malformed-standard-builtin-impl");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        home.join("std/str/index.nct"),
        r#"instance str {
    pub method self.consume(): usize { return 0 }
}
"#,
    )
    .unwrap();
    crate::test_files::write(root.join("index.nct"), "func main(): i32 { return 0 }\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0417"),
        "{diagnostics:?}"
    );
}

#[test]
fn check_resolves_types_imported_by_builtin_method_signatures() {
    let root = make_temp_project("builtin-method-imported-return");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        home.join("std/iter/index.nct"),
        r#"pub struct Iter<T> {
    pub marker: usize
}

instance Iter<T> {
    pub method &self.value(): usize { return self.marker }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/str/index.nct"),
        r#"use std/iter.Iter

instance str {
    pub method &self.bytes_iter(): Iter<u8> {
        return Iter<u8> { marker: 42 }
    }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        r#"func inspect(): usize {
    let iterator = "".bytes_iter()
    return iterator.value()
}

func main(): i32 { return 0 }
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
