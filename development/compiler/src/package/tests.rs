use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn loads_named_package_and_separate_executable_entry() {
    let root = temp_package("named");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("nocter.nct"),
        r#"#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    entry: "./src/app",
}
"#,
    )
    .unwrap();
    fs::write(root.join("src/app.nct"), "func main(): i32 { 0 }\n").unwrap();

    let load = load_package(&root);
    assert!(load.diagnostics.is_empty(), "{:?}", load.diagnostics);
    let package = load.package.unwrap();
    assert_eq!(package.display_name(), "json-tool");
    assert_eq!(package.version(), Some("0.1.0"));
    assert_eq!(package.executables().len(), 1);
    assert_eq!(package.executables()[0].name(), "json-tool");
    assert_eq!(package.executables()[0].id().package(), package.id());
    assert_eq!(package.executables()[0].id().name(), "json-tool");
    let ModuleKey::Path(path) = package.executables()[0].entry().id().key() else {
        panic!("separate executable entry must have a module path")
    };
    assert_eq!(path.as_str(), "./src/app");
    assert_eq!(
        package.executables()[0].entry().source_path(),
        package.root().join("src/app.nct")
    );
}

#[test]
fn unnamed_package_uses_directory_only_as_display_name() {
    let root = temp_package("fallback-name");
    fs::write(root.join("nocter.nct"), "").unwrap();
    let package = load_package(&root).package.unwrap();
    assert_eq!(
        package.display_name(),
        root.file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(package.id(), &PackageId::root(&root));
}

#[test]
fn omitted_entry_selects_the_package_root_module() {
    let root = temp_package("root-executable");
    fs::write(
        root.join("nocter.nct"),
        r#"#executable: {
    name: "app",
}

func main(): i32 { 0 }
"#,
    )
    .unwrap();
    let package = load_package(&root).package.unwrap();
    assert_eq!(
        package.executables()[0].entry().id().key(),
        &ModuleKey::PackageRoot
    );
    assert_eq!(
        package.executables()[0].entry().source_path(),
        package.package_file_path()
    );
}

#[test]
fn dot_entry_selects_the_root_index_module() {
    let root = temp_package("root-index-entry");
    fs::write(
        root.join("nocter.nct"),
        "#executable: { name: \"app\", entry: \".\" }\n",
    )
    .unwrap();
    fs::write(root.join("index.nct"), "func main(): i32 { 0 }\n").unwrap();

    let package = load_package(&root).package.unwrap();
    let ModuleKey::Path(path) = package.executables()[0].entry().id().key() else {
        panic!("index entry must have a module path")
    };
    assert_eq!(path.as_str(), ".");
    assert_eq!(
        package.executables()[0].entry().source_path(),
        package.root().join("index.nct")
    );
}

#[test]
fn equivalent_entry_spellings_share_one_module_identity() {
    let root = temp_package("normalized-entry-identity");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("nocter.nct"),
        r#"#executable: { name: "first", entry: "./src/app" }
#executable: { name: "second", entry: "./src//app" }
"#,
    )
    .unwrap();
    fs::write(root.join("src/app.nct"), "func main(): i32 { 0 }\n").unwrap();

    let package = load_package(&root).package.unwrap();
    assert_eq!(
        package.executables()[0].entry().id(),
        package.executables()[1].entry().id()
    );
    let ModuleKey::Path(path) = package.executables()[1].entry().id().key() else {
        panic!("explicit entry must have a module path")
    };
    assert_eq!(path.as_str(), "./src/app");
}

#[test]
fn dot_entry_does_not_fall_back_to_the_package_file() {
    let root = temp_package("missing-root-index-entry");
    fs::write(
        root.join("nocter.nct"),
        r#"#executable: { name: "app", entry: "." }

func main(): i32 { 0 }
"#,
    )
    .unwrap();

    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not exist at `index.nct`")),
        "{:?}",
        load.diagnostics
    );
}

#[test]
fn resolved_executable_id_uses_the_loaded_package_identity() {
    let root = temp_package("resolved-package-id");
    fs::write(
        root.join("nocter.nct"),
        "#executable: { name: \"app\" }\n\nfunc main(): i32 { 0 }\n",
    )
    .unwrap();
    let expected = PackageId::from_descriptor("resolved-package-id");

    let package = loader::load_package_with_id(&root, Some(expected.clone()))
        .package
        .unwrap();
    assert_eq!(package.id(), &expected);
    assert_eq!(package.executables()[0].id().package(), &expected);
    assert_eq!(package.executables()[0].entry().id().package(), &expected);
}

#[test]
fn rejects_duplicate_executable_names_and_unknown_fields() {
    let root = temp_package("invalid-fields");
    fs::write(root.join("app.nct"), "func main(): i32 { 0 }\n").unwrap();
    fs::write(
        root.join("nocter.nct"),
        r#"#executable: {
    name: "app",
    entry: "./app",
}
#executable: {
    name: "app",
    entry: "./app",
    module: "legacy",
}
"#,
    )
    .unwrap();
    let load = load_package(&root);
    let messages = load
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&"unknown executable field `module`"));

    fs::write(
        root.join("nocter.nct"),
        r#"#executable: { name: "app", entry: "./app" }
#executable: { name: "app", entry: "./app" }
"#,
    )
    .unwrap();
    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate executable name `app`")
    );
}

#[test]
fn rejects_unknown_package_directives() {
    let root = temp_package("unknown-directive");
    fs::write(root.join("nocter.nct"), "#edition: \"future\"\n").unwrap();
    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unknown package directive `#edition`")
    );
}

#[test]
fn rejects_entry_suffix_escape_missing_and_ambiguity() {
    for (name, entry, expected) in [
        ("suffix", "./app.nct", "without a `.nct` suffix"),
        ("escape", "../app", "beginning with `./`"),
        ("missing", "./missing", "does not exist"),
    ] {
        let root = temp_package(name);
        fs::write(
            root.join("nocter.nct"),
            format!("#executable: {{\n    name: \"app\",\n    entry: \"{entry}\",\n}}\n"),
        )
        .unwrap();
        let load = load_package(&root);
        assert!(
            load.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{:?}",
            load.diagnostics
        );
    }

    let root = temp_package("ambiguous");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app.nct"), "").unwrap();
    fs::write(root.join("app/index.nct"), "").unwrap();
    fs::write(
        root.join("nocter.nct"),
        "#executable: { name: \"app\", entry: \"./app\" }\n",
    )
    .unwrap();
    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is ambiguous"))
    );
}

#[cfg(unix)]
#[test]
fn rejects_executable_entry_symlinks_that_escape_the_package_root() {
    use std::os::unix::fs::symlink;

    let root = temp_package("symlink-escape");
    let outside = temp_package("symlink-escape-target").join("app.nct");
    fs::write(&outside, "func main(): i32 { 0 }\n").unwrap();
    symlink(&outside, root.join("app.nct")).unwrap();
    fs::write(
        root.join("nocter.nct"),
        "#executable: { name: \"app\", entry: \"./app\" }\n",
    )
    .unwrap();

    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("escapes the package root")),
        "{:?}",
        load.diagnostics
    );
}

#[test]
fn rejects_entries_that_cross_into_a_nested_package() {
    let root = temp_package("nested-package-entry");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(
        root.join("nocter.nct"),
        "#executable: { name: \"app\", entry: \"./nested/app\" }\n",
    )
    .unwrap();
    fs::write(root.join("nested/nocter.nct"), "").unwrap();
    fs::write(root.join("nested/app.nct"), "func main(): i32 { 0 }\n").unwrap();

    let load = load_package(&root);
    assert!(
        load.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("crosses into the nested package")),
        "{:?}",
        load.diagnostics
    );
}

#[cfg(unix)]
#[test]
fn rejects_package_file_symlinks_that_escape_the_root() {
    use std::os::unix::fs::symlink;

    let root = temp_package("manifest-symlink-escape");
    let outside = temp_package("manifest-symlink-target").join("nocter.nct");
    fs::write(&outside, "").unwrap();
    symlink(&outside, root.join("nocter.nct")).unwrap();

    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("nocter.nct")
                && diagnostic.message.contains("escapes the root")),
        "{:?}",
        load.diagnostics
    );
}

fn temp_package(name: &str) -> PathBuf {
    let unique = format!(
        "nocter-package-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}
