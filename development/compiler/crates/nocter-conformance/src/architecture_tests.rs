use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("conformance crate is inside the compiler workspace")
        .to_path_buf()
}

fn manifest(crate_name: &str) -> String {
    let path = workspace()
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn production_dependencies(crate_name: &str) -> BTreeSet<String> {
    let manifest = manifest(crate_name);
    let Some((_, dependencies)) = manifest.split_once("[dependencies]\n") else {
        return BTreeSet::new();
    };
    dependencies
        .split("\n[")
        .next()
        .expect("dependency section exists")
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
        .collect()
}

fn production_dependency_closure(crate_name: &str) -> BTreeSet<String> {
    fn visit(crate_name: &str, closure: &mut BTreeSet<String>) {
        for dependency in production_dependencies(crate_name) {
            if closure.insert(dependency.clone())
                && workspace()
                    .join("crates")
                    .join(&dependency)
                    .join("Cargo.toml")
                    .is_file()
            {
                visit(&dependency, closure);
            }
        }
    }

    let mut closure = BTreeSet::new();
    visit(crate_name, &mut closure);
    closure
}

fn crate_names() -> Vec<String> {
    let mut names = fs::read_dir(workspace().join("crates"))
        .expect("compiler crate directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn core_program_layers_keep_the_reviewed_dependency_direction() {
    let expected = [
        ("nocter-persistent", &[][..]),
        ("nocter-language", &[][..]),
        (
            "nocter-model",
            &["nocter-language", "nocter-persistent"][..],
        ),
        ("nocter-declarations", &["nocter-model"][..]),
        (
            "nocter-target-program",
            &[
                "nocter-checking",
                "nocter-declarations",
                "nocter-model",
                "nocter-runtime-contract",
            ][..],
        ),
        (
            "nocter-mir",
            &[
                "nocter-checking",
                "nocter-model",
                "nocter-runtime-contract",
                "nocter-target-program",
            ][..],
        ),
        (
            "nocter-machine",
            &["nocter-mir", "nocter-model", "nocter-runtime-contract"][..],
        ),
        (
            "nocter-arm64",
            &["nocter-machine", "nocter-runtime-contract"][..],
        ),
        ("nocter-macho", &["nocter-arm64", "nocter-hash"][..]),
    ];
    for (crate_name, allowed) in expected {
        let actual = production_dependencies(crate_name);
        let allowed = allowed
            .iter()
            .map(|dependency| (*dependency).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, allowed,
            "review production dependencies for {crate_name}"
        );
    }
}

#[test]
fn semantic_editor_stack_does_not_inherit_native_backend_layers() {
    let forbidden = [
        "nocter-arm64",
        "nocter-machine",
        "nocter-macho",
        "nocter-mir",
        "nocter-native-session",
    ];
    for crate_name in [
        "nocter-session",
        "nocter-analysis",
        "nocter-language-server",
    ] {
        let closure = production_dependency_closure(crate_name);
        let inherited = forbidden
            .iter()
            .filter(|dependency| closure.contains(**dependency))
            .collect::<Vec<_>>();
        assert!(
            inherited.is_empty(),
            "{crate_name} inherits native backend crates: {inherited:?}"
        );
    }
}

#[test]
fn language_server_consumes_analysis_queries_not_semantic_storage() {
    let dependencies = production_dependencies("nocter-language-server");
    for forbidden in [
        "nocter-checking",
        "nocter-declarations",
        "nocter-source-index",
        "nocter-target-program",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "language-server protocol code must not consume {forbidden} directly"
        );
    }
}

#[test]
fn persistent_storage_has_only_reviewed_semantic_authority_consumers() {
    let allowed = BTreeSet::from(["nocter-checking", "nocter-model"]);
    for crate_name in crate_names() {
        if production_dependencies(&crate_name).contains("nocter-persistent") {
            assert!(
                allowed.contains(crate_name.as_str()),
                "{crate_name} must consume an immutable semantic contract, not persistent storage"
            );
        }
    }
}

#[test]
fn neutral_handoff_contracts_do_not_depend_on_editor_projection() {
    for crate_name in ["nocter-compile-input", "nocter-frontend-bindings"] {
        let dependencies = production_dependencies(crate_name);
        assert!(
            !dependencies.contains("nocter-source-index"),
            "{crate_name} must hand off syntax identities without editor projection"
        );
    }
}

#[test]
fn builtin_type_vocabulary_is_one_rust_type() {
    fn accepts_language_builtin(_: nocter_language::BuiltinType) {}

    accepts_language_builtin(nocter_model::BuiltinType::Bool);
    accepts_language_builtin(nocter_syntax::BuiltinType::Bool);
}
