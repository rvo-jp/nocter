use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn check_loads_non_relative_std_imports_from_nocter_home() {
    let root = make_temp_project("std-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/io.answer

func main(): i32 {
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
        r#"use std/io as io

func main(): i32 {
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
fn check_loads_std_fmt_import_graph_from_nocter_home() {
    let root = make_temp_project("std-fmt-import-graph");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/fmt.append_i32

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/error.nct"),
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/mem.nct"),
        r#"pub struct Allocator {
    state: usize
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/string.nct"),
        r#"use std/mem.Allocator

pub struct String {
    ptr: *u8
    len: usize
    cap: usize
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/fmt.nct"),
        r#"use std/error.Error
use std/string.String

pub func append_i32(out: &+String, value: i32): void! {
    return
}

pub func unsupported(): error {
    return Error.new("std.fmt.unsupported", "value cannot be formatted")
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
        r#"use std/io.title

func main(): i32 {
    return title()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func title(): &str {
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
fn check_loads_std_imports_from_common_std() {
    let root = make_temp_project("std-import-common");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/io.answer

func main(): i32 {
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
fn check_ignores_inactive_target_items_in_common_std() {
    let root = make_temp_project("std-inactive-target-items");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/io.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"#target("x64-linux")
pub(nocter) primitive unknown_for_linux(): void

#target("x64-linux")
pub func answer(): &str {
    return "inactive"
}

#target("x64-linux")
pub type RawAnswer = &str

#target("x64-linux")
pub copy struct Handle {
    pub raw: &str
}

#target("x64-linux")
pub enum Status {
    inactive
}

pub type RawAnswer = i32

pub copy struct Handle {
    pub raw: i32
}

pub enum Status {
    active
}

pub func answer(): i32 {
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
        r#"use std/ptr.internal

func main(): i32 {
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
        r#"use std/ptr.internal

func main(): i32 {
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
fn check_rejects_primitive_declaration_outside_nocter_home_std() {
    let root = make_temp_project("user-primitive");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"primitive new_error(code: &str, message: &str): error

func main(): i32 {
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
    assert_eq!(diagnostics[0].code, "E0414");
    assert!(
        diagnostics[0].message.contains("primitive"),
        "{diagnostics:?}"
    );
}

#[test]
fn check_rejects_unregistered_primitive_declaration_inside_nocter_home_std() {
    let root = make_temp_project("unregistered-primitive");
    let home = make_nocter_home(&root);
    fs::write(
        home.join("std/error.nct"),
        r#"primitive not_registered(): error
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(home.join("std/error.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0415");
    assert!(
        diagnostics[0].message.contains("not_registered"),
        "{diagnostics:?}"
    );
}

#[test]
fn check_reports_missing_non_relative_imports() {
    let root = make_temp_project("missing-std-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/missing.answer

func main(): i32 {
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

func main(): i32 {
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
