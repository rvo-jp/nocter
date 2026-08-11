use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;
use std::path::Path;

#[test]
fn check_synthesizes_standard_prelude_for_user_modules() {
    let root = make_temp_project("synthetic-prelude");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/prelude/index.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/prelude_helpers/index.nct"),
        r#"pub func answer(value: i32): i32 {
    return value
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0320");
}

#[test]
fn check_synthetic_prelude_ignores_package_root_shadow() {
    let root = make_temp_project("synthetic-prelude-home-only");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("std/prelude")).unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        r#"func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("std/prelude/index.nct"),
        r#"pub func wrong(): i32 {
    return 1
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/prelude/index.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/prelude_helpers/index.nct"),
        r#"pub func answer(): i32 {
    return 7
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
fn check_explicit_top_level_name_overrides_prelude_fallback() {
    let root = make_temp_project("synthetic-prelude-top-level-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    crate::test_files::write(
        root.join("index.nct"),
        r#"func answer(): i32 {
    return 1
}

func main(): i32 {
    return answer()
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
fn check_explicit_import_overrides_the_same_prelude_export() {
    let root = make_temp_project("synthetic-prelude-explicit-import");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use std/prelude_helpers.answer

func main(): i32 {
    return answer()
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
fn check_parameter_name_overrides_prelude_fallback() {
    let root = make_temp_project("synthetic-prelude-parameter-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    crate::test_files::write(
        root.join("index.nct"),
        r#"func consume(answer: i32): i32 {
    return answer
}

func main(): i32 {
    return consume(1)
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
fn check_local_binding_overrides_prelude_fallback() {
    let root = make_temp_project("synthetic-prelude-local-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    crate::test_files::write(
        root.join("index.nct"),
        r#"func main(): i32 {
    let answer = 1
    return answer
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
fn check_block_import_overrides_prelude_fallback() {
    let root = make_temp_project("synthetic-prelude-block-import-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    crate::test_files::write(
        root.join("index.nct"),
        r#"func main(): i32 {
    use std/math.answer
    return answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/math/index.nct"),
        r#"pub func answer(): i32 {
    return 7
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

fn write_answer_prelude(home: &Path) {
    crate::test_files::write(
        home.join("std/prelude/index.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/prelude_helpers/index.nct"),
        r#"pub func answer(): i32 {
    return 7
}
"#,
    )
    .unwrap();
}
