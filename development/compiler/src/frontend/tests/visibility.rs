use super::support::{check_with_nocter_home, make_nocter_home, make_temp_project};
use crate::source::SourceMap;
use std::fs;

#[test]
fn ancestor_visibility_admits_exactly_the_selected_module_tree() {
    let root = make_temp_project("ancestor-visibility");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("nocter.nct"), "#name: \"visibility\"\n").unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        "use /internal/a/child.child_value\nuse /internal/b.sibling_value\nuse /app.package_value\n\nfunc main(): i32 { return child_value() + sibling_value() + package_value() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/a/index.nct"),
        "pub(./) func descendants(): i32 { return 1 }\npub(../) func internal_tree(): i32 { return 2 }\npub(/) func package_tree(): i32 { return 3 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/a/child/index.nct"),
        "use /internal/a.descendants\n\npub func child_value(): i32 { return descendants() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/b/index.nct"),
        "use /internal/a.internal_tree\n\npub func sibling_value(): i32 { return internal_tree() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("app/index.nct"),
        "use /internal/a.package_tree\n\npub func package_value(): i32 { return package_tree() }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn ancestor_visibility_rejects_siblings_outside_the_boundary() {
    let root = make_temp_project("ancestor-visibility-rejection");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("nocter.nct"), "#name: \"visibility\"\n").unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        "use /outside.run\n\nfunc main(): i32 { return run() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/a/index.nct"),
        "pub(./) func descendants(): i32 { return 1 }\npub(../) func internal_tree(): i32 { return 2 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("outside/index.nct"),
        "use /internal/a.{descendants, internal_tree}\n\npub func run(): i32 { return descendants() + internal_tree() }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0412")
            .count(),
        2,
        "{diagnostics:?}"
    );
}

#[test]
fn visibility_parent_components_cannot_cross_the_package_root() {
    let root = make_temp_project("visibility-above-root");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("nocter.nct"), "#name: \"visibility\"\n").unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        "use /child\n\nfunc main(): i32 { return 0 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("child/index.nct"),
        "pub(../../) func invalid(): void { return }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0420" && diagnostic.message.contains("package root")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn scoped_reexports_preserve_the_reexporting_module_as_the_visibility_origin() {
    let root = make_temp_project("scoped-reexport-chain");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("nocter.nct"), "#name: \"visibility\"\n").unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        "use /internal/facade/child/grandchild.run\n\nfunc main(): i32 { return run() }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/a/index.nct"),
        "pub(../) func internal_value(): i32 { return 7 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/facade/index.nct"),
        "pub(./) use /internal/a.internal_value\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/facade/child/index.nct"),
        "pub(./) use /internal/facade.internal_value\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/facade/child/grandchild/index.nct"),
        "use /internal/facade/child.internal_value\n\npub func run(): i32 { return internal_value() }\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reexports_cannot_widen_an_ancestor_visibility_boundary() {
    let root = make_temp_project("widening-reexport");
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("nocter.nct"), "#name: \"visibility\"\n").unwrap();
    crate::test_files::write(
        root.join("index.nct"),
        "use /internal/facade\n\nfunc main(): i32 { return 0 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/a/index.nct"),
        "pub(../) func internal_value(): i32 { return 7 }\n",
    )
    .unwrap();
    crate::test_files::write(
        root.join("internal/facade/index.nct"),
        "pub(/) use /internal/a.internal_value\n",
    )
    .unwrap();

    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let diagnostics = check_with_nocter_home(&mut sources, source, &home);
    fs::remove_dir_all(&root).unwrap();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0412" && diagnostic.message.contains("would widen")
        }),
        "{diagnostics:?}"
    );
}
