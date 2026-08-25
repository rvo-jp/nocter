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

fn source(relative: &str) -> String {
    fs::read_to_string(workspace().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

fn production_dependencies(crate_name: &str) -> BTreeSet<String> {
    let manifest = source(&format!("crates/{crate_name}/Cargo.toml"));
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

#[test]
fn core_program_layers_keep_the_reviewed_dependency_direction() {
    let expected = [
        ("nocter-language", &[][..]),
        ("nocter-model", &["nocter-language"][..]),
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
                "nocter-declarations",
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
fn semantic_authorities_do_not_read_editor_projection_contracts() {
    let standard = source("crates/nocter-checking/src/standard_semantics.rs");
    assert!(!standard.contains("nocter_source_index"));
    assert!(!standard.contains("FrontendBindings"));

    let primitives = source("crates/nocter-declaration-lowering/src/primitive_bindings.rs");
    assert!(!primitives.contains("nocter_source_index"));
    assert!(primitives.contains("FrontendBindings"));

    let target = source("crates/nocter-target-program/src/program.rs");
    assert!(!target.contains("validate_package_targets"));

    for path in [
        "crates/nocter-declaration-lowering/src/surface.rs",
        "crates/nocter-declaration-lowering/src/reservation.rs",
        "crates/nocter-declaration-lowering/src/definitions/mod.rs",
    ] {
        assert!(!source(path).contains("TargetSelection::prepare"));
    }
}

#[test]
fn editor_mutations_select_semantic_identities_instead_of_standard_api_spellings() {
    let action = source("crates/nocter-analysis/src/code_actions/conformance.rs");
    assert!(!action.contains("std/process.abort"));
    assert!(!action.contains("completion.label() != \"abort\""));
    assert!(action.contains("StandardDeclarationRole::ProcessAbort"));
    assert!(action.contains("SemanticEntity::Callable(terminator)"));
}

#[test]
fn builtin_type_vocabulary_has_one_definition() {
    let roots = [
        "crates/nocter-language/src/lib.rs",
        "crates/nocter-model/src/type_store.rs",
        "crates/nocter-syntax/src/token.rs",
    ];
    let definitions = roots
        .iter()
        .map(|path| source(path).matches("pub enum BuiltinType").count())
        .sum::<usize>();
    assert_eq!(definitions, 1);
}
