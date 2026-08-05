use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn loads_named_package_and_separate_executable_module() {
    let root = temp_package("named");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("index.nct"),
        r#"#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    module: "./src/app",
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
    assert_eq!(package.executables()[0].id().package(), PackageId::ROOT);
    assert_eq!(package.executables()[0].id().name(), "json-tool");
    assert_eq!(
        package.executables()[0].module().logical_path(),
        "./src/app"
    );
    assert_eq!(
        package.executables()[0].source_path(),
        package.root().join("src/app.nct")
    );
}

#[test]
fn unnamed_package_uses_directory_only_as_display_name() {
    let root = temp_package("fallback-name");
    fs::write(root.join("index.nct"), "").unwrap();
    let package = load_package(&root).package.unwrap();
    assert_eq!(
        package.display_name(),
        root.file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(package.id(), PackageId::ROOT);
}

#[test]
fn root_module_is_an_explicit_executable_target() {
    let root = temp_package("root-executable");
    fs::write(
        root.join("index.nct"),
        r#"#executable: {
    name: "app",
    module: ".",
}

func main(): i32 { 0 }
"#,
    )
    .unwrap();
    let package = load_package(&root).package.unwrap();
    assert_eq!(package.executables()[0].source_path(), package.index_path());
}

#[test]
fn rejects_duplicate_executable_names_and_unknown_fields() {
    let root = temp_package("invalid-fields");
    fs::write(root.join("app.nct"), "func main(): i32 { 0 }\n").unwrap();
    fs::write(
        root.join("index.nct"),
        r#"#executable: {
    name: "app",
    module: "./app",
}
#executable: {
    name: "app",
    module: "./app",
    entry: "start",
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
    assert!(messages.contains(&"unknown executable field `entry`"));

    fs::write(
        root.join("index.nct"),
        r#"#executable: { name: "app", module: "./app" }
#executable: { name: "app", module: "./app" }
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
    fs::write(root.join("index.nct"), "#edition: \"future\"\n").unwrap();
    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unknown package directive `#edition`")
    );
}

#[test]
fn rejects_module_suffix_escape_missing_and_ambiguity() {
    for (name, module, expected) in [
        ("suffix", "./app.nct", "without a `.nct` suffix"),
        ("escape", "../app", "beginning with `./`"),
        ("missing", "./missing", "does not exist"),
    ] {
        let root = temp_package(name);
        fs::write(
            root.join("index.nct"),
            format!("#executable: {{\n    name: \"app\",\n    module: \"{module}\",\n}}\n"),
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
        root.join("index.nct"),
        "#executable: { name: \"app\", module: \"./app\" }\n",
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
fn rejects_executable_module_symlinks_that_escape_the_package_root() {
    use std::os::unix::fs::symlink;

    let root = temp_package("symlink-escape");
    let outside = temp_package("symlink-escape-target").join("app.nct");
    fs::write(&outside, "func main(): i32 { 0 }\n").unwrap();
    symlink(&outside, root.join("app.nct")).unwrap();
    fs::write(
        root.join("index.nct"),
        "#executable: { name: \"app\", module: \"./app\" }\n",
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

#[cfg(unix)]
#[test]
fn rejects_package_index_symlinks_that_escape_the_root() {
    use std::os::unix::fs::symlink;

    let root = temp_package("index-symlink-escape");
    let outside = temp_package("index-symlink-target").join("index.nct");
    fs::write(&outside, "").unwrap();
    symlink(&outside, root.join("index.nct")).unwrap();

    let load = load_package(&root);
    assert!(
        load.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index.nct")
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
