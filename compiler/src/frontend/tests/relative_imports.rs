use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_loads_relative_imports() {
    let root = make_temp_project("relative-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import answer

program(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
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
fn check_uses_relative_imported_function_return_type() {
    let root = make_temp_project("relative-import-return-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import title

program(): i32 {
    return title()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
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
fn check_uses_relative_imported_function_parameters() {
    let root = make_temp_project("relative-import-parameters");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import answer

program(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
        r#"pub func answer(value: i32): i32 {
    return value
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0320");
}

#[test]
fn check_uses_relative_imported_associated_function_return_type() {
    let root = make_temp_project("relative-import-associated-function");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./geometry import Point

program(): i32 {
    return Point.origin().x
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("geometry.nct"),
        r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }
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
fn check_uses_relative_imported_method_return_type() {
    let root = make_temp_project("relative-import-method");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./geometry import Point

program(): i32 {
    let point = Point.origin()
    return point.x_value()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("geometry.nct"),
        r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }

    pub method (point: Self).x_value(): i32 {
        return point.x
    }
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
fn check_reports_relative_imported_function_body_errors() {
    let root = make_temp_project("relative-import-function-body-error");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
        r#"pub func answer(): i32 {
    return "bad"
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn check_reports_relative_imported_impl_member_name_duplicates() {
    let root = make_temp_project("relative-import-impl-duplicate");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./geometry import Point

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("geometry.nct"),
        r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }

    pub method (point: Self).origin(): i32 {
        return point.x
    }
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0413");
}

#[test]
fn check_reports_missing_relative_imported_names() {
    let root = make_temp_project("missing-relative-imported-name");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import missing

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
        r#"func answer(): i32 {
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
    assert_eq!(diagnostics[0].code, "E0411");
}

#[test]
fn check_reports_private_relative_imported_names() {
    let root = make_temp_project("private-relative-imported-name");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
        r#"func answer(): i32 {
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
    assert!(diagnostics[0].message.contains("private"));
}

#[test]
fn check_reports_relative_import_parse_errors() {
    let root = make_temp_project("relative-import-parse-error");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(root.join("config.nct"), "module config\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0200");
}

#[test]
fn check_reports_missing_relative_imports() {
    let root = make_temp_project("missing-relative-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"from ./missing import Missing

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
