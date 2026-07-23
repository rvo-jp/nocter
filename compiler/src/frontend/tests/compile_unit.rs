use super::super::{FrontendOptions, load_compile_unit};
use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::analysis::analyze_compile_unit_with_entry;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::fs;

#[test]
fn compile_unit_analysis_retains_per_file_results() {
    let root = make_temp_project("compile-unit-analysis");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use ./config.answer

func main(): i32 {
    return answer()
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
    let options = FrontendOptions {
        nocter_home: Some(home.to_path_buf()),
        source_root: None,
        target: DEFAULT_TARGET.to_string(),
    };
    let unit = load_compile_unit(&mut sources, source, &options).unwrap();
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(analysis.files.iter().filter(|file| file.is_root).count(), 1);
    assert!(
        analysis.files.iter().any(|file| file.is_root
            && file.ast.span.source == source
            && file.diagnostics.is_empty())
    );

    let config = analysis
        .files
        .iter()
        .find(|file| {
            sources
                .get(file.ast.span.source)
                .and_then(|source_file| source_file.absolute_path())
                .map(|path| path.ends_with("config.nct"))
                .unwrap_or(false)
        })
        .expect("expected config.nct analysis");
    assert!(config.resolved.symbols.symbol_by_name("answer").is_some());
    assert!(
        config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0312")
    );

    assert_eq!(
        analysis
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0312")
            .count(),
        1
    );
}

#[test]
fn compile_unit_reuses_preloaded_import_source() {
    let root = make_temp_project("compile-unit-preloaded-import");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use ./config.answer

func main(): i32 {
    return answer()
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config.nct"),
        r#"pub func answer(): i32 {
    return 0
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let root_source = sources.load_file(root.join("app.nct")).unwrap();
    let config_path = root.join("config.nct").canonicalize().unwrap();
    let config_display = config_path.to_string_lossy().into_owned();
    let config_source = sources.add_source(
        config_display,
        Some(config_path),
        r#"pub func answer(): i32 {
    return "bad"
}
"#,
    );
    let options = FrontendOptions {
        nocter_home: Some(home.to_path_buf()),
        source_root: None,
        target: DEFAULT_TARGET.to_string(),
    };
    let unit = load_compile_unit(&mut sources, root_source, &options).unwrap();
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
    fs::remove_dir_all(&root).unwrap();

    let config = analysis
        .files
        .iter()
        .find(|file| file.ast.span.source == config_source)
        .expect("expected preloaded config source in compile unit");
    assert!(
        config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0312")
    );
}

#[test]
fn target_gated_type_imports_follow_active_target() {
    let root = make_temp_project("target-gated-type-imports");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"use std/os.PlatformWord

func main(): i32 {
    return 0
}
"#,
    )
    .unwrap();
    fs::write(
        home.join("std/os.nct"),
        r#"#target("arm64-darwin")
pub copy struct PlatformWord {
    pub value: usize
}
"#,
    )
    .unwrap();

    let diagnostics_for_target = |target: &str| {
        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let options = FrontendOptions {
            nocter_home: Some(home.to_path_buf()),
            source_root: None,
            target: target.to_string(),
        };
        let unit = load_compile_unit(&mut sources, source, &options).unwrap();
        analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME).diagnostics()
    };

    let matching_diagnostics = diagnostics_for_target(DEFAULT_TARGET);
    let mismatched_diagnostics = diagnostics_for_target("x64-linux");
    fs::remove_dir_all(&root).unwrap();

    assert!(matching_diagnostics.is_empty(), "{matching_diagnostics:?}");
    assert_eq!(mismatched_diagnostics.len(), 1);
    assert_eq!(mismatched_diagnostics[0].code, "E0411");
    assert!(
        mismatched_diagnostics[0]
            .message
            .contains("does not export `PlatformWord`"),
        "{mismatched_diagnostics:?}"
    );
}

#[test]
fn check_orders_diagnostics_by_source_position() {
    let root = make_temp_project("diagnostic-order");
    let home = make_nocter_home(&root);
    fs::write(
        root.join("app.nct"),
        r#"func takes_i32(value: i32): i32 {
    return value
}

func main(): i32 {
    return "bad"
}

func later(): i32 {
    return takes_i32("bad")
}
"#,
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("app.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0312");
    assert_eq!(diagnostics[1].code, "E0321");

    let first_span = diagnostics[0].primary_span.as_ref().unwrap();
    let second_span = diagnostics[1].primary_span.as_ref().unwrap();
    assert!(first_span.start_byte < second_span.start_byte);
}
