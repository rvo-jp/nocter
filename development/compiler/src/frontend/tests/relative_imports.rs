use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_composes_same_module_source_files() {
    let root = make_temp_project("same-module-source-import");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./search

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("search.nct"),
        r#"func answer(): i32 {
    return 1
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
fn check_rejects_public_declarations_in_an_implementation_source() {
    let root = make_temp_project("public-implementation-source");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("index.nct"), "use ./search\n").unwrap();
    crate::test_files::write(root.join("search.nct"), "pub func answer(): i32 { 1 }\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0421");
}

#[test]
fn check_composes_cyclic_same_module_source_graph_once() {
    let root = make_temp_project("cyclic-same-module-sources");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        "use ./left\n\nfunc main(): i32 { return left() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("left.nct"),
        "use ./right\n\nfunc left(): i32 { return right() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("right.nct"),
        "use ./left\n\nfunc right(): i32 { return 42 }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_reports_duplicate_declarations_across_same_module_sources() {
    let root = make_temp_project("duplicate-same-module-sources");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        "use ./left\nuse ./right\n\nfunc main(): i32 { return 0 }\n",
    )
    .unwrap();
    crate::test_files::write(root.join("left.nct"), "func answer(): i32 { return 1 }\n").unwrap();
    crate::test_files::write(root.join("right.nct"), "func answer(): i32 { return 2 }\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0400");
}

#[test]
fn check_bare_use_loads_relative_namespace_exports() {
    let root = make_temp_project("bare-relative-use");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config

func main(): i32 {
    return config.answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
        r#"pub func answer(): i32 {
    return 1
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
fn check_loads_publicly_reexported_namespace() {
    let root = make_temp_project("public-relative-namespace-reexport");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("api")).unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./api

func main(): i32 {
    return api.json.answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(root.join("api/index.nct"), "pub use ./json\n").unwrap();
    crate::test_files::write(
        root.join("api/json/index.nct"),
        "pub func answer(): i32 { 7 }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_directory_index_namespace_uses_directory_name() {
    let root = make_temp_project("relative-directory-namespace-use");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./path/to/dir

func main(): i32 {
    return dir.answer()
}
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("path/to/dir")).unwrap();
    crate::test_files::write(
        root.join("path/to/dir/index.nct"),
        r#"pub func answer(): i32 {
    return 1
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
fn check_uses_relative_imported_function_return_type() {
    let root = make_temp_project("relative-import-return-type");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.title

func main(): i32 {
    return title()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
        r#"pub func title(): &str {
    return "Nocter"
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
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn check_uses_relative_imported_function_parameters() {
    let root = make_temp_project("relative-import-parameters");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
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
fn check_uses_relative_imported_associated_function_return_type() {
    let root = make_temp_project("relative-import-associated-function");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./geometry.Point

func main(): i32 {
    return Point.origin().x
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("geometry/index.nct"),
        r#"pub struct Point {
    pub x: i32
}

construct Point {
    pub default func origin(): Self {
        return Point { x: 0 }
    }
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
fn check_uses_relative_imported_method_return_type() {
    let root = make_temp_project("relative-import-method");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./geometry.Point

func main(): i32 {
    let point = Point.origin()
    return point.x_value()
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("geometry/index.nct"),
        r#"pub struct Point {
    pub x: i32
}

construct Point {
    pub default func origin(): Self {
        return Point { x: 0 }
    }
}

instance Point {
    pub method self.x_value(): i32 {
        return self.x
    }
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
fn check_reports_relative_imported_function_body_errors() {
    let root = make_temp_project("relative-import-function-body-error");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.answer

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
        r#"pub func answer(): i32 {
    return "bad"
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert!(diagnostics[0].message.contains("i32"));
    assert!(diagnostics[0].message.contains("str"));
}

#[test]
fn check_reports_relative_imported_method_owner_member_name_duplicates() {
    let root = make_temp_project("relative-import-impl-duplicate");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./geometry.Point

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("geometry/index.nct"),
        r#"pub struct Point {
    pub x: i32
}

construct Point {
    pub default func origin(): Self {
        return Point { x: 0 }
    }
}

instance Point {
    pub method self.origin(): i32 {
        return self.x
    }
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0413");
}

#[test]
fn check_reports_missing_relative_imported_names() {
    let root = make_temp_project("missing-relative-imported-name");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.missing

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
        r#"func answer(): i32 {
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
    assert_eq!(diagnostics[0].code, "E0411");
}

#[test]
fn check_reports_private_relative_imported_names() {
    let root = make_temp_project("private-relative-imported-name");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.answer

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        root.join("config/index.nct"),
        r#"func answer(): i32 {
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
    assert_eq!(diagnostics[0].code, "E0412");
    assert!(diagnostics[0].message.contains("private"));
}

#[test]
fn check_reports_relative_import_parse_errors() {
    let root = make_temp_project("relative-import-parse-error");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./config.answer

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    crate::test_files::write(root.join("config/index.nct"), "module config\n").unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0200");
}

#[test]
fn check_reports_missing_relative_imports() {
    let root = make_temp_project("missing-relative-import");
    let home = make_nocter_home(&root);
    crate::test_files::write(
        root.join("index.nct"),
        r#"use ./missing.Missing

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0410");
}
