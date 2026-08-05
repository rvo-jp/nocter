use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::diagnostics::Diagnostic;
use crate::source::SourceMap;
use std::fs;
use std::path::Path;

#[test]
fn check_synthesizes_standard_prelude_for_user_modules() {
    let root = make_temp_project("synthetic-prelude");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/prelude.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/prelude_helpers.nct"),
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
fn check_synthetic_prelude_ignores_package_root_shadow() {
    let root = make_temp_project("synthetic-prelude-home-only");
    let home = make_nocter_home(&root);
    fs::create_dir_all(root.join("std")).unwrap();
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("std/prelude.nct"),
        r#"pub func wrong(): i32 {
    return 1
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/prelude.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/prelude_helpers.nct"),
        r#"pub func answer(): i32 {
    return 7
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
fn check_reports_top_level_prelude_collision() {
    let root = make_temp_project("synthetic-prelude-top-level-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    fs::write(
        root.join("app.nct"),
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
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_prelude_collision(&diagnostics);
}

#[test]
fn check_reports_parameter_prelude_collision() {
    let root = make_temp_project("synthetic-prelude-parameter-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    fs::write(
        root.join("app.nct"),
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
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_prelude_collision(&diagnostics);
}

#[test]
fn check_reports_local_binding_prelude_collision() {
    let root = make_temp_project("synthetic-prelude-local-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    let answer = 1
    return answer
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_prelude_collision(&diagnostics);
}

#[test]
fn check_reports_block_import_prelude_collision() {
    let root = make_temp_project("synthetic-prelude-block-import-collision");
    let home = make_nocter_home(&root);
    write_answer_prelude(&home);
    fs::write(
        root.join("app.nct"),
        r#"func main(): i32 {
    use std/math.answer
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/math.nct"),
        r#"pub func answer(): i32 {
    return 7
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_prelude_collision(&diagnostics);
}

fn write_answer_prelude(home: &Path) {
    fs::write(
        home.join("std/prelude.nct"),
        r#"pub use std/prelude_helpers.answer
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/prelude_helpers.nct"),
        r#"pub func answer(): i32 {
    return 7
}
"#,
    )
    .unwrap();
}

fn assert_prelude_collision(diagnostics: &[Diagnostic]) {
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0400");
    assert!(
        diagnostics[0]
            .message
            .contains("synthetic standard prelude"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics[0]
            .notes
            .iter()
            .any(|note| note.message.contains("prelude introduces")),
        "{diagnostics:?}"
    );
}
