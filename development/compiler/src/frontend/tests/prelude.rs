use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

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
fn check_synthetic_prelude_ignores_source_root_shadow() {
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
