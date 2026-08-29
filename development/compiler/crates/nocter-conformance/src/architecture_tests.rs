use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use nocter_json::{Member, Value};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("conformance crate is inside the compiler workspace")
        .to_path_buf()
}

fn object_member<'value>(members: &'value [Member], name: &str) -> &'value Value {
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .unwrap_or_else(|| panic!("cargo metadata object has no {name} member"))
        .value
}

fn metadata_dependency_graph() -> &'static BTreeMap<String, BTreeSet<String>> {
    static GRAPH: OnceLock<BTreeMap<String, BTreeSet<String>>> = OnceLock::new();
    GRAPH.get_or_init(|| {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .current_dir(workspace())
            .output()
            .expect("run cargo metadata for architecture tests");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = nocter_json::parse(
            std::str::from_utf8(&output.stdout).expect("cargo metadata is UTF-8"),
        )
        .expect("cargo metadata is valid JSON");
        let Value::Object(root) = metadata else {
            panic!("cargo metadata root is an object");
        };
        let Value::Array(packages) = object_member(&root, "packages") else {
            panic!("cargo metadata packages is an array");
        };
        packages
            .iter()
            .map(|package| {
                let Value::Object(package) = package else {
                    panic!("cargo metadata package is an object");
                };
                let Value::String(name) = object_member(package, "name") else {
                    panic!("cargo metadata package name is a string");
                };
                let Value::Array(dependencies) = object_member(package, "dependencies") else {
                    panic!("cargo metadata dependencies is an array");
                };
                let dependencies = dependencies
                    .iter()
                    .filter_map(|dependency| {
                        let Value::Object(dependency) = dependency else {
                            panic!("cargo metadata dependency is an object");
                        };
                        if !matches!(object_member(dependency, "kind"), Value::Null) {
                            return None;
                        }
                        let Value::String(name) = object_member(dependency, "name") else {
                            panic!("cargo metadata dependency name is a string");
                        };
                        Some(name.to_string())
                    })
                    .collect();
                (name.to_string(), dependencies)
            })
            .collect()
    })
}

fn production_dependencies(crate_name: &str) -> BTreeSet<String> {
    metadata_dependency_graph()
        .get(crate_name)
        .unwrap_or_else(|| panic!("cargo metadata has no package {crate_name}"))
        .clone()
}

fn production_dependency_closure(crate_name: &str) -> BTreeSet<String> {
    fn visit(crate_name: &str, closure: &mut BTreeSet<String>) {
        for dependency in production_dependencies(crate_name) {
            if closure.insert(dependency.clone())
                && metadata_dependency_graph().contains_key(&dependency)
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
    metadata_dependency_graph().keys().cloned().collect()
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
        (
            "nocter-declarations",
            &["nocter-model", "nocter-toolchain-contract"][..],
        ),
        (
            "nocter-target-program",
            &[
                "nocter-checking",
                "nocter-declarations",
                "nocter-model",
                "nocter-runtime-contract",
                "nocter-toolchain-contract",
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
        "nocter-workspace-analysis",
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
        "nocter-compile-input",
        "nocter-declarations",
        "nocter-discovery",
        "nocter-model",
        "nocter-package",
        "nocter-session",
        "nocter-source-index",
        "nocter-syntax",
        "nocter-target-program",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "language-server protocol code must not consume {forbidden} directly"
        );
    }
}

#[test]
fn editor_orchestration_layers_keep_the_reviewed_dependency_boundary() {
    let expected = [
        (
            "nocter-workspace-analysis",
            &[
                "nocter-analysis",
                "nocter-compile-input",
                "nocter-diagnostics",
                "nocter-discovery",
                "nocter-filesystem",
                "nocter-model",
                "nocter-package",
                "nocter-session",
                "nocter-source",
                "nocter-workspace-revision",
            ][..],
        ),
        (
            "nocter-language-server",
            &[
                "nocter-analysis",
                "nocter-diagnostics",
                "nocter-filesystem",
                "nocter-json",
                "nocter-lsp",
                "nocter-source",
                "nocter-workspace-analysis",
                "nocter-workspace-revision",
            ][..],
        ),
    ];
    for (crate_name, dependencies) in expected {
        let expected = dependencies
            .iter()
            .map(|dependency| (*dependency).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            production_dependencies(crate_name),
            expected,
            "review every new production dependency for {crate_name}"
        );
    }
}

#[test]
fn workspace_revision_owns_source_transitions_without_semantic_dependencies() {
    assert_eq!(
        production_dependencies("nocter-workspace-revision"),
        BTreeSet::from(["nocter-filesystem".to_owned()])
    );
}

#[test]
fn computation_kernel_has_no_compiler_domain_dependency() {
    assert_eq!(
        production_dependencies("nocter-computation"),
        BTreeSet::from(["nocter-hash".to_owned()])
    );
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
fn toolchain_handoff_vocabulary_stays_below_declaration_construction() {
    assert!(
        production_dependencies("nocter-toolchain-contract").is_empty(),
        "toolchain identities must remain a dependency-free closed vocabulary"
    );
    for crate_name in ["nocter-compile-input", "nocter-discovery"] {
        let closure = production_dependency_closure(crate_name);
        assert!(
            !closure.contains("nocter-declarations"),
            "{crate_name} must not recover toolchain identities from declaration storage"
        );
    }
}

#[test]
fn builtin_type_vocabulary_is_one_rust_type() {
    fn accepts_language_builtin(_: nocter_language::BuiltinType) {}

    accepts_language_builtin(nocter_model::BuiltinType::Bool);
    accepts_language_builtin(nocter_syntax::BuiltinType::Bool);
}
