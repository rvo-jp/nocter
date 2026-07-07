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
        r#"pub from std/prelude_helpers import answer
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
