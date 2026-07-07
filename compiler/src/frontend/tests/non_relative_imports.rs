use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_loads_non_relative_std_imports_from_nocter_home() {
    let root = make_temp_project("std-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from std/io import answer

program(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func answer(): i32 {
    return 1
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
fn check_loads_namespace_imports_from_nocter_home() {
    let root = make_temp_project("std-namespace-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"import std/io as io

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func answer(): i32 {
    return 1
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
fn check_uses_non_relative_imported_function_return_type() {
    let root = make_temp_project("std-import-return-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from std/io import title

program(): i32 {
    return title()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func title(): str {
    return "Nocter"
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
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn check_prefers_target_overlay_for_std_imports() {
    let root = make_temp_project("std-import-overlay");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from std/io import answer

program(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func answer(): str {
    return "common"
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("targets/arm64-darwin/std/io.nct"),
        r#"pub func answer(): i32 {
    return 1
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
fn check_reports_nocter_visibility_import_from_user_project() {
    let root = make_temp_project("nocter-visibility-user-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from std/ptr import internal

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/ptr.nct"),
        r#"pub(nocter) func internal(): i32 {
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
    assert_eq!(diagnostics[0].code, "E0412");
    assert!(diagnostics[0].message.contains("pub(nocter)"));
}

#[test]
fn check_allows_nocter_visibility_import_inside_nocter_home() {
    let root = make_temp_project("nocter-visibility-home-import");
    let home = make_nocter_home(&root);
    fs::write(
        home.join("std/io.nct"),
        r#"from std/ptr import internal

program(): i32 {
    return internal()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/ptr.nct"),
        r#"pub(nocter) func internal(): i32 {
    return 1
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(home.join("std/io.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_reports_missing_non_relative_imports() {
    let root = make_temp_project("missing-std-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from std/missing import answer

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0410");
}

#[test]
fn check_loads_non_relative_use_imports() {
    let root = make_temp_project("std-use");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/prelude

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(home.join("std/prelude.nct"), "module prelude\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0200");
}
