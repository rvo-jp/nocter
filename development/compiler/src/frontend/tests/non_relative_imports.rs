use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
fn check_loads_block_scope_non_relative_selected_imports() {
    let root = make_temp_project("block-std-selected-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    use std/math.answer
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/math.nct"),
        r#"pub func answer(): i32 {
    return 42
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
fn check_loads_block_scope_non_relative_namespace_imports() {
    let root = make_temp_project("block-std-namespace-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    use std/math
    return math.answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/math.nct"),
        r#"pub func answer(): i32 {
    return 42
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
fn check_bare_use_loads_non_relative_namespace_exports() {
    let root = make_temp_project("bare-std-use");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/math

func main(): i32 {
    return math.answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/math.nct"),
        r#"pub func answer(): i32 {
    return 42
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
fn check_bare_use_does_not_import_non_relative_exports_directly() {
    let root = make_temp_project("bare-std-use-no-direct-export");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/math

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/math.nct"),
        r#"pub func answer(): i32 {
    return 42
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0416");
    assert!(diagnostics[0].message.contains("`answer`"));
}

#[test]
fn check_loads_non_relative_imports_from_source_root_before_nocter_home() {
    let root = make_temp_project("source-root-import");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("app.nct"),
        r#"use lib/math.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/math.nct"),
        r#"pub func answer(): i32 {
    return 7
}
"#,
    )
    .unwrap();
    fs::create_dir_all(home.join("lib")).unwrap();
    fs::write(
        home.join("lib/math.nct"),
        r#"pub func answer(): &str {
    return "wrong root"
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
fn check_source_root_std_shadows_nocter_home_std() {
    let root = make_temp_project("source-root-std-shadow");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("std")).unwrap();
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
        root.join("std/io.nct"),
        r#"pub func answer(): i32 {
    return 1
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/io.nct"),
        r#"pub func answer(): &str {
    return "home"
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
fn check_loads_directory_index_module() {
    let root = make_temp_project("directory-index-import");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("lib/math")).unwrap();
    fs::write(
        root.join("app.nct"),
        r#"use lib/math.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/math/index.nct"),
        r#"pub func answer(): i32 {
    return 3
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
fn check_reports_ambiguous_file_and_directory_index_module() {
    let root = make_temp_project("ambiguous-index-import");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("lib/math")).unwrap();
    fs::write(
        root.join("app.nct"),
        r#"use lib/math.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/math.nct"),
        r#"pub func answer(): i32 {
    return 1
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/math/index.nct"),
        r#"pub func answer(): i32 {
    return 2
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
    assert!(
        diagnostics[0]
            .message
            .contains("ambiguous import `lib/math`"),
        "{diagnostics:?}"
    );
}

#[test]
fn check_prefers_file_module_when_directory_has_no_index() {
    let root = make_temp_project("file-module-with-empty-directory");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("lib/math")).unwrap();
    fs::write(
        root.join("app.nct"),
        r#"use lib/math.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib/math.nct"),
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
fn check_loads_absolute_imports() {
    let root = make_temp_project("absolute-import-project");
    let home = make_nocter_home(&root);
    let absolute_root = absolute_import_root();
    fs::create_dir_all(absolute_root.join("shared")).unwrap();
    let import_path = absolute_root.join("shared/answer");
    let import_path_text = import_path.to_string_lossy().into_owned();
    fs::write(
        root.join("app.nct"),
        format!(
            r#"use {import_path_text}.answer

func main(): i32 {{
    return answer()
}}
"#
        ),
    )
    .unwrap();
    fs::write(
        absolute_root.join("shared/answer.nct"),
        r#"pub func answer(): i32 {
    return 9
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&absolute_root).unwrap();

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
fn check_uses_imported_function_return_type_with_imported_signature_type() {
    let root = make_temp_project("std-import-return-imported-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/process.args

func main(): i32! {
    let values = args()?
    let count: usize = values.len()
    return 0
}
"#,
    )
    .unwrap();
    write_std_process_args_implementation(&home, "process.nct");
    write_std_vec_with_len_method(&home);

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_namespace_use_preserves_imported_signature_type_dependencies() {
    let root = make_temp_project("std-bare-use-return-imported-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/process

func main(): i32! {
    let values = process.args()?
    let count: usize = values.len()
    return 0
}
"#,
    )
    .unwrap();
    write_std_process_args_implementation(&home, "process.nct");
    write_std_vec_with_len_method(&home);

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_reexport_preserves_imported_signature_type_dependencies() {
    let root = make_temp_project("std-reexport-return-imported-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/process.args

func main(): i32! {
    let values = args()?
    let count: usize = values.len()
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/process.nct"),
        r#"pub use std/process_impl.args
"#,
    )
    .unwrap();
    write_std_process_args_implementation(&home, "process_impl.nct");
    write_std_vec_with_len_method(&home);

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_chained_reexport_preserves_imported_signature_type_dependencies() {
    let root = make_temp_project("std-chained-reexport-return-imported-type");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/process.args

func main(): i32! {
    let values = args()?
    let count: usize = values.len()
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/process.nct"),
        r#"pub use std/process_exports.args
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/process_exports.nct"),
        r#"pub use std/process_impl.args
"#,
    )
    .unwrap();
    write_std_process_args_implementation(&home, "process_impl.nct");
    write_std_vec_with_len_method(&home);

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn check_reexport_cycle_reports_missing_export() {
    let root = make_temp_project("std-reexport-cycle");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/a.answer

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/a.nct"),
        r#"pub use std/b.answer
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/b.nct"),
        r#"pub use std/a.answer
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E0411")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("`std/a` does not export `answer`"),
        "{diagnostics:?}"
    );
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
fn check_reports_nocter_field_access_from_user_project() {
    let root = make_temp_project("nocter-field-user-access");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/mem.{Raw, make}

func main(): i32 {
    let raw = make()
    return raw.value
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/mem.nct"),
        r#"pub struct Raw {
    pub(nocter) value: i32
}

pub func make(): Raw {
    return Raw { value: 1 }
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0377");
    assert!(diagnostics[0].message.contains("not visible here"));
}

#[test]
fn check_allows_nocter_field_access_inside_nocter_home() {
    let root = make_temp_project("nocter-field-home-access");
    let home = make_nocter_home(&root);
    fs::write(
        home.join("std/io.nct"),
        r#"use std/mem.{Raw, make}

func main(): i32 {
    let raw = make()
    return raw.value
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/mem.nct"),
        r#"pub struct Raw {
    pub(nocter) value: i32
}

pub func make(): Raw {
    return Raw { value: 1 }
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
fn check_reports_nocter_method_call_from_user_project() {
    let root = make_temp_project("nocter-method-user-call");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/mem.make

func main(): i32 {
    let raw = make()
    return raw.secret()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/mem.nct"),
        r#"pub struct Raw {
    value: i32
}

pub func make(): Raw {
    return Raw { value: 1 }
}

impl Raw {
    pub(nocter) method &self.secret(): i32 {
        return self.value
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
    assert_eq!(diagnostics[0].code, "E0389");
    assert!(diagnostics[0].message.contains("has no method `secret`"));
}

#[test]
fn check_allows_nocter_method_call_inside_nocter_home() {
    let root = make_temp_project("nocter-method-home-call");
    let home = make_nocter_home(&root);
    fs::write(
        home.join("std/io.nct"),
        r#"use std/mem.make

func main(): i32 {
    let raw = make()
    return raw.secret()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/mem.nct"),
        r#"pub struct Raw {
    value: i32
}

pub func make(): Raw {
    return Raw { value: 1 }
}

impl Raw {
    pub(nocter) method &self.secret(): i32 {
        return self.value
    }
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
fn check_rejects_source_level_prelude_imports() {
    let root = make_temp_project("std-prelude-source-use");
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

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0200");
    assert!(
        diagnostics[0]
            .message
            .contains("`std/prelude` is compiler-managed")
    );
}

fn absolute_import_root() -> PathBuf {
    let unique = format!(
        "nocter_absolute_import_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    PathBuf::from("/tmp").join(unique)
}

fn write_std_process_args_implementation(home: &Path, file_name: &str) {
    fs::write(
        home.join("std").join(file_name),
        r#"use std/vec.Vec

pub func args(): Vec<&str>! {
    return Vec.empty()
}
"#,
    )
    .unwrap();
}

fn write_std_vec_with_len_method(home: &Path) {
    fs::write(
        home.join("std/vec.nct"),
        r#"pub struct Vec<T> {
    pub len: usize
}

pub func Vec.empty<T>(): Vec<T> {
    return Vec<T> { len: 0 }
}

pub func len<T>(values: &Vec<T>): usize {
    return values.len
}

impl<T> Vec<T> {
    pub method &self.len(): usize {
        return len(self)
    }
}
"#,
    )
    .unwrap();
}
