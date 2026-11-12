//! Real owned multi-source input; structural pairs never stand in for resolved header equality.
use super::*;
use crate::test_support::source_see;
use crate::{DeclarationContractRule, SurfaceDeclarationId, SurfaceDeclarationKind};
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode,
};
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_target_selection::TargetSelection;
use std::sync::Arc;

pub(super) fn inventory(
    files: &[(&str, ModuleSourceKind, &str)],
    edges: &[(usize, usize, usize)],
    reverse: bool,
) -> ModuleSources<'static> {
    let mut sources = SourceMap::new();
    let trees: Arc<[SyntaxTree]> = files
        .iter()
        .map(|(path, _, text)| {
            let id = sources
                .add_bytes(SourceName::new(*path), text.as_bytes())
                .unwrap();
            let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
            assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
            tree
        })
        .collect::<Vec<_>>()
        .into();
    let selected = TargetSelection::prepare_for_goal(
        ParseGoal::V2CoreUnit,
        CompilationTarget::Arm64Darwin,
        &sources,
        trees.iter(),
    )
    .unwrap();
    let mut inputs = files
        .iter()
        .enumerate()
        .map(|(index, (path, role, _))| {
            ModuleSourceInput::shared(*path, *role, Arc::clone(&trees), index).unwrap()
        })
        .collect::<Vec<_>>();
    if reverse {
        inputs.reverse();
    }
    let resolutions = edges
        .iter()
        .map(|(source, occurrence, target)| {
            source_see(&trees[*source], *occurrence, files[*target].0)
        })
        .collect();
    let package = PackageIdentity::new("workspace:app");
    let input = CompileUnitInput::from_shared_target_selection(
        CompilationTarget::Arm64Darwin,
        Arc::new(sources),
        vec![PackageInput::new(
            package.clone(),
            "app",
            PackageMode::Declared,
        )],
        vec![ModuleInput::new(
            ModuleIdentity::new(package, Vec::<&str>::new()),
            inputs,
        )],
        Vec::new(),
        selected,
    )
    .with_source_visibility_resolutions(resolutions);
    ModuleSources::prepare(&input).unwrap()
}
fn id(
    pair: &PairedModuleSources<'_>,
    path: &str,
    kind: SurfaceDeclarationKind,
    name: &str,
) -> SurfaceDeclarationId {
    let sources = pair.inventory();
    sources
        .declaration_entries()
        .find(|(_, declaration)| {
            let source = &sources.sources()[declaration.source().index()];
            source.canonical_path() == path
                && declaration.kind() == kind
                && declaration.name().and_then(|token| {
                    sources
                        .source_map()
                        .get(token.source())?
                        .text_at(token.range())
                }) == Some(name)
        })
        .unwrap()
        .0
}
fn rule(
    result: Result<PairedModuleSources<'_>, ModuleSourcesFailure>,
    expected: DeclarationContractRule,
) {
    let Err(ModuleSourcesFailure::Contract(diagnostic)) = result else {
        panic!("{result:?}")
    };
    assert_eq!(diagnostic.rule(), expected);
}

#[test]
fn root_contracts_pair_without_pub_and_keep_actual_representation_field_identities() {
    use ModuleSourceKind::{Implementation, Root};
    use SurfaceDeclarationKind::{Constant, Field, Function, Static, Struct};
    for reverse in [false, true] {
        let paired = inventory(&[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Item { visible: i32\nlast: bool }\nfunc Item(value: i32): i32\nconst LIMIT: i32\nstatic VALUE: i32"),
            ("/app/body.nct", Implementation, "see ./index.nct\nstruct Item { hidden: i32\nvisible: i32\nmiddle: i32\nlast: bool }\nfunc Item(value: i32): i32 { value }\nconst LIMIT: i32 = 42\nstatic VALUE: i32 = 7"),
        ], &[(0, 0, 1), (1, 0, 0)], reverse).pair().unwrap();
        let contracts = paired.contracts();
        for (kind, name) in [
            (Function, "Item"),
            (Constant, "LIMIT"),
            (Static, "VALUE"),
            (Struct, "Item"),
            (Field, "visible"),
            (Field, "last"),
        ] {
            let contract = id(&paired, "/app/index.nct", kind, name);
            let definition = id(&paired, "/app/body.nct", kind, name);
            assert_eq!(contracts.representative(definition), contract);
            if kind == Struct {
                assert_eq!(contracts.representation(contract), Some(definition));
            }
        }
        let hidden = id(&paired, "/app/body.nct", Field, "hidden");
        assert_eq!(contracts.representative(hidden), hidden);
    }
}

#[test]
fn physical_pairing_is_direct_reciprocal_and_never_inferred_from_equal_headers() {
    use ModuleSourceKind::{Implementation, Root};
    for edges in [&[(0, 0, 1)][..], &[(0, 0, 1), (1, 0, 2), (2, 0, 0)][..]] {
        let files = if edges.len() == 1 {
            vec![
                ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
                ("/app/body.nct", Implementation, "func run(): i32 { 42 }"),
            ]
        } else {
            vec![
                ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
                (
                    "/app/body.nct",
                    Implementation,
                    "see ./bridge.nct\nfunc run(): i32 { 42 }",
                ),
                ("/app/bridge.nct", Implementation, "see ./index.nct"),
            ]
        };
        rule(
            inventory(&files, edges, false).pair(),
            DeclarationContractRule::MissingBody,
        );
    }
}

#[test]
fn repeated_headers_and_visible_field_order_are_checked_before_type_resolution() {
    use ModuleSourceKind::{Implementation, Root};
    for (contract, definition, expected) in [
        (
            "func run(value: Missing): Missing",
            "func run(other: Missing): Missing { other }",
            DeclarationContractRule::MismatchedBody,
        ),
        (
            "func run(edit value: Missing): Missing",
            "func run(value: Missing): Missing { value }",
            DeclarationContractRule::MismatchedBody,
        ),
        (
            "func run(): [i32; if true { 1 } else { 2 }]",
            "func run(): [i32; if true { 1 } else { 3 }] { [42] }",
            DeclarationContractRule::MismatchedBody,
        ),
        (
            "const VALUE: i32",
            "const VALUE: i64 = 42",
            DeclarationContractRule::MismatchedCompileTimeInitializer,
        ),
        (
            "struct Item { a: i32\nb: i32 }",
            "struct Item { b: i32\na: i32 }",
            DeclarationContractRule::MismatchedRepresentation,
        ),
        (
            "struct Item { a: i32 }",
            "struct Item { a: i64 }",
            DeclarationContractRule::MismatchedRepresentation,
        ),
        (
            "copy struct Item",
            "struct Item { a: i32 }",
            DeclarationContractRule::MismatchedRepresentation,
        ),
    ] {
        let root = format!("see ./body.nct\n{contract}");
        let body = format!("see ./index.nct\n{definition}");
        let selected = inventory(
            &[
                ("/app/index.nct", Root, &root),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
            false,
        );
        let root_source = selected
            .sources()
            .iter()
            .find(|source| source.canonical_path() == "/app/index.nct")
            .unwrap()
            .syntax()
            .source();
        let body_source = selected
            .sources()
            .iter()
            .find(|source| source.canonical_path() == "/app/body.nct")
            .unwrap()
            .syntax()
            .source();
        let result = selected.pair();
        let Err(ModuleSourcesFailure::Contract(diagnostic)) = result else {
            panic!("{contract}: {result:?}")
        };
        assert_eq!(diagnostic.rule(), expected, "{contract}");
        assert_eq!(diagnostic.source().primary().source(), body_source);
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(
            diagnostic.source().notes()[0].origin().source(),
            root_source
        );
    }
}

#[test]
fn missing_nominal_definitions_and_duplicate_bodies_are_not_private_overload_fallbacks() {
    use ModuleSourceKind::{Implementation, Root};
    rule(
        inventory(
            &[("/app/index.nct", Root, "struct Item { value: i32 }")],
            &[],
            false,
        )
        .pair(),
        DeclarationContractRule::MissingRepresentation,
    );
    for reverse in [false, true] {
        rule(
            inventory(
                &[
                    (
                        "/app/index.nct",
                        Root,
                        "see ./a.nct\nsee ./b.nct\nfunc run(): i32",
                    ),
                    (
                        "/app/a.nct",
                        Implementation,
                        "see ./index.nct\nfunc run(): i32 { 1 }",
                    ),
                    (
                        "/app/b.nct",
                        Implementation,
                        "see ./index.nct\nfunc run(): i32 { 2 }",
                    ),
                ],
                &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (2, 0, 0)],
                reverse,
            )
            .pair(),
            DeclarationContractRule::DuplicateBody,
        );
    }
}

#[test]
fn an_exact_signature_cannot_hide_a_second_definition_of_the_same_contract_identity() {
    use ModuleSourceKind::{Implementation, Root};
    for (contract, first, second, expected) in [
        (
            "func run(value: i32): i32",
            "func run(value: i32): i32 { value }",
            "func run(other: i64): i32 { 0 }",
            DeclarationContractRule::DuplicateBody,
        ),
        (
            "const VALUE: i32",
            "const VALUE: i32 = 42",
            "const VALUE: i64 = 7",
            DeclarationContractRule::DuplicateCompileTimeInitializer,
        ),
        (
            "struct Item",
            "struct Item { value: i32 }",
            "copy struct Item { value: i32 }",
            DeclarationContractRule::DuplicateRepresentation,
        ),
    ] {
        let root = format!("see ./a.nct\nsee ./b.nct\n{contract}");
        let a = format!("see ./index.nct\n{first}");
        let b = format!("see ./index.nct\n{second}");
        for reverse in [false, true] {
            rule(
                inventory(
                    &[
                        ("/app/index.nct", Root, &root),
                        ("/app/a.nct", Implementation, &a),
                        ("/app/b.nct", Implementation, &b),
                    ],
                    &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (2, 0, 0)],
                    reverse,
                )
                .pair(),
                expected,
            );
        }
    }
}
