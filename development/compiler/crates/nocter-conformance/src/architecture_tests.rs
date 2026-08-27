use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    fn collect(directory: &Path, sources: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).expect("Rust source directory") {
            let path = entry.expect("Rust source entry").path();
            if path.is_dir() {
                collect(&path, sources);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }

    let mut sources = Vec::new();
    collect(root, &mut sources);
    sources.sort();
    sources
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
fn semantic_features_cannot_acquire_an_unsealed_query_context() {
    let analysis = workspace().join("crates/nocter-analysis/src");
    let query = analysis.join("query");
    let kernel = fs::read_to_string(query.join("mod.rs")).expect("semantic query kernel");
    assert!(
        kernel.contains("fn unvalidated_semantic_query"),
        "the raw query constructor must remain an explicit kernel boundary"
    );
    assert!(
        !kernel.contains("pub(crate) fn unvalidated_semantic_query")
            && !kernel.contains("pub(in crate) fn unvalidated_semantic_query"),
        "an unsealed semantic context must not be visible to feature modules"
    );
    assert!(
        kernel.contains("validate_generation(self.sources(), self.syntax_trees())"),
        "the public crate query path must seal the complete source projection"
    );

    for path in rust_sources(&analysis) {
        if path.starts_with(&query)
            || path.file_name().and_then(|name| name.to_str()) == Some("tests.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("analysis source");
        for forbidden in [
            "SemanticQueryContext",
            "CompleteSemanticQuery",
            "nocter_checking",
            "nocter_source_index",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} consumes query-kernel representation through {forbidden}",
                path.display()
            );
        }
    }
}

#[test]
fn session_semantics_have_one_stage_graph_and_one_query_handoff() {
    let compiler = workspace().join("crates");
    let session = compiler.join("nocter-session/src");
    let pipeline_path = session.join("semantic_pipeline.rs");
    for path in rust_sources(&session) {
        if path == pipeline_path
            || path.components().any(|part| part.as_os_str() == "tests")
            || path.file_name().and_then(|name| name.to_str()) == Some("tests.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("session production source");
        for stage_entry in [
            "lower_compile_unit_declarations_recovering(",
            "lower_incomplete_body_declarations_recovering(",
            "prepare_program_checking_recovering(",
            "prepare_analysis_program_checking_recovering(",
            "check_prepared_program_recovering(",
            "analyze_prepared_program_bodies(",
        ] {
            assert!(
                !source.contains(stage_entry),
                "{} bypasses the session's single semantic pipeline through {stage_entry}",
                path.display()
            );
        }
    }

    let query = fs::read_to_string(compiler.join("nocter-analysis/src/query/mod.rs"))
        .expect("semantic query kernel");
    for forbidden in [
        "AnalysisState",
        "CurrentSemanticEvidence",
        "CompiledTarget",
        "SemanticEvidenceBundle",
        "target.program().checked()",
    ] {
        assert!(
            !query.contains(forbidden),
            "query kernel reconstructs session storage through {forbidden}"
        );
    }
    assert!(
        query.contains("evidence: self.semantic_evidence()?"),
        "query kernel must consume the snapshot's single session-evidence handoff"
    );
}

#[test]
fn source_projection_integrity_cannot_fail_semantic_construction() {
    for relative in ["nocter-declaration-lowering/src", "nocter-checking/src"] {
        let root = workspace().join("crates").join(relative);
        for path in rust_sources(&root) {
            let source = fs::read_to_string(&path).expect("semantic production source");
            for forbidden in [
                "DuplicateSourceBinding",
                "DuplicateDocumentation",
                "SourceProjectionIssue",
                ".issues()",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{} lets editor projection integrity affect semantics through {forbidden}",
                    path.display()
                );
            }
        }
    }

    let index = fs::read_to_string(workspace().join("crates/nocter-source-index/src/index.rs"))
        .expect("source projection builder");
    assert!(
        index.contains("issues: Vec<SourceProjectionIssue>"),
        "source projection builder must own its integrity report"
    );
    assert!(
        !index.contains("Result<(), DuplicateSourceBinding>")
            && !index.contains("Result<(), DuplicateDocumentation>"),
        "source projection insertion must not return a semantic-pipeline failure"
    );
}

#[test]
fn workspace_analysis_never_chooses_authority_by_order() {
    let analysis =
        fs::read_to_string(workspace().join("crates/nocter-language-server/src/analysis.rs"))
            .expect("workspace analysis owner");
    assert!(
        analysis.contains("AmbiguousDocumentAnalysis"),
        "multiple current contexts need an explicit typed outcome"
    );
    assert!(
        analysis.contains("ScopeCompilationInput")
            && analysis.contains("for source in requested_sources"),
        "package analysis must derive roots from its complete source demand"
    );
    let input = fs::read_to_string(
        workspace().join("crates/nocter-language-server/src/analysis/compilation_input.rs"),
    )
    .expect("workspace compilation input");
    assert!(
        input.contains("requested_sources: Box<[PathBuf]>")
            && input.contains("collect::<BTreeSet<_>>()"),
        "package compilation input must canonicalize the complete source demand"
    );
    for forbidden in [
        "source_scope_priority",
        "min_by_key(|scope|",
        "scope_members[0]",
    ] {
        assert!(
            !analysis.contains(forbidden),
            "workspace analysis still chooses semantic authority by order through {forbidden}"
        );
    }
}

#[test]
fn query_caches_do_not_derive_a_second_semantic_domain() {
    let path = workspace().join("crates/nocter-analysis/src/query/session.rs");
    let source = fs::read_to_string(&path).expect("analysis query session");
    for forbidden in [
        "AnalysisState",
        "CurrentSemanticEvidence",
        "interruption_count",
    ] {
        assert!(
            !source.contains(forbidden),
            "query cache must not inspect semantic storage through {forbidden}"
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
