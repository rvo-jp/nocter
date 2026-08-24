use nocter_model::{BuiltinType, ParameterOrigin, TypeKind};
use nocter_source::SourceMap;
use nocter_syntax::{NodeKind, ParseGoal};

use super::test_support::{add_source, all_nodes, bind, module, package, parse_source};
use super::{TypeNormalizationError, TypeNormalizationRule, normalize_header_types};
use crate::{ModuleIdentity, PackageIdentity, evaluate_header_constants};

fn normalized_app<'syntax>(
    sources: &'syntax SourceMap,
    app_manifest: &'syntax nocter_syntax::SyntaxTree,
    app: &'syntax nocter_syntax::SyntaxTree,
    std_manifest: &'syntax nocter_syntax::SyntaxTree,
    std_root: &'syntax nocter_syntax::SyntaxTree,
    prelude: &'syntax nocter_syntax::SyntaxTree,
) -> Result<super::PreparedTypes<'syntax>, TypeNormalizationError> {
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", app_manifest),
            package("builtin:std", "std", "/std/index.nct", std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", app),
            module("builtin:std", &[], "/std/index.nct", std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                prelude,
            ),
        ],
        vec![],
        &prelude_identity,
    )
    .unwrap();
    normalize_header_types(evaluate_header_constants(bound).unwrap())
}

fn fixture(
    sources: &mut SourceMap,
    text: &str,
) -> (
    nocter_syntax::SyntaxTree,
    nocter_syntax::SyntaxTree,
    nocter_syntax::SyntaxTree,
    nocter_syntax::SyntaxTree,
    nocter_syntax::SyntaxTree,
) {
    let app_manifest_id = add_source(sources, "/app/index.nct", "");
    let std_manifest_id = add_source(sources, "/std/index.nct", "");
    let app_id = add_source(sources, "/app/index.nct", text);
    let std_root_id = add_source(sources, "/std/index.nct", "");
    let prelude_id = add_source(sources, "/std/prelude/index.nct", "");
    (
        parse_source(sources, app_manifest_id, ParseGoal::SourceFile),
        parse_source(sources, app_id, ParseGoal::SourceFile),
        parse_source(sources, std_manifest_id, ParseGoal::SourceFile),
        parse_source(sources, std_root_id, ParseGoal::SourceFile),
        parse_source(sources, prelude_id, ParseGoal::SourceFile),
    )
}

#[test]
fn expands_generic_aliases_without_creating_canonical_alias_types() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut sources,
        "type View<T> = &[T]\nfunc read(values: View<i32>): void { return }\n",
    );
    let normalized = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();
    let store = normalized
        .namespaces()
        .imports
        .generics
        .headers
        .reserved
        .program
        .types();

    let expanded = all_nodes(&app, NodeKind::Type)
        .into_iter()
        .filter_map(|node| normalized.type_for(node))
        .find(|ty| {
            let Some(TypeKind::Borrow { referent, .. }) = store.get(*ty) else {
                return false;
            };
            let Some(TypeKind::Slice(element)) = store.get(*referent) else {
                return false;
            };
            store.get(*element) == Some(&TypeKind::Builtin(BuiltinType::I32))
        });
    assert!(expanded.is_some());
}

#[test]
fn type_equality_requires_an_associated_projection_after_alias_expansion() {
    let mut invalid_sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut invalid_sources,
        "func equality<T, U>(): T where T = U { return }\n",
    );
    let error = normalized_app(
        &invalid_sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap_err();
    let TypeNormalizationError::Rule(violation) = error else {
        panic!("projection-free equality was not an authored normalization rule")
    };
    assert_eq!(
        violation.rule(),
        TypeNormalizationRule::EqualityWithoutAssociatedProjection
    );
    assert!(matches!(
        violation.primary(),
        nocter_source_index::SyntaxOrigin::Node(node)
            if app.node(node).is_some_and(|syntax| syntax.kind() == NodeKind::TypeEqualityPredicate)
    ));

    let mut valid_sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut valid_sources,
        concat!(
            "interface Source {\n    pub type Item\n}\n",
            "type ItemOf<T> = T.Item where T: Source\n",
            "func equality<T, U>(): T where T: Source, ItemOf<T> = U { return }\n",
        ),
    );
    normalized_app(
        &valid_sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();
}

#[test]
fn resolves_interface_generic_and_concrete_associated_selections_to_one_identity() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut sources,
        concat!(
            "struct Buffer {}\n",
            "interface Source {\n",
            "    pub type Item\n",
            "    pub method &self.get(): Self.Item\n",
            "}\n",
            "conform Source for Buffer {\n",
            "    type Item = i32\n",
            "    method &self.get(): i32 { return 0 }\n",
            "}\n",
            "func generic<S>(source: &S): S.Item where S: Source { return source.get() }\n",
            "func concrete(source: &Buffer): Buffer.Item { return source.get() }\n",
        ),
    );
    let normalized = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();
    let store = normalized
        .namespaces()
        .imports
        .generics
        .headers
        .reserved
        .program
        .types();
    let projections: Vec<_> = all_nodes(&app, NodeKind::Type)
        .into_iter()
        .filter_map(|node| normalized.type_for(node))
        .filter_map(|ty| match store.get(ty) {
            Some(TypeKind::AssociatedProjection { associated, .. }) => Some(associated),
            _ => None,
        })
        .collect();

    assert!(projections.len() >= 3);
    assert!(projections.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn infers_the_single_named_structural_callable_origin() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) =
        fixture(&mut sources, "type View = &func(value: &str): &str\n");
    let normalized = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();
    let store = normalized
        .namespaces()
        .imports
        .generics
        .headers
        .reserved
        .program
        .types();
    let callable = normalized
        .type_for(all_nodes(&app, NodeKind::Type)[0])
        .and_then(|ty| match store.get(ty) {
            Some(TypeKind::Callable(callable)) => Some(callable),
            _ => None,
        })
        .expect("alias target is a callable type");

    assert_eq!(callable.provenance().origins(), &[ParameterOrigin::new(0)]);
}

#[test]
fn rejects_recursive_alias_expansion() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) =
        fixture(&mut sources, "type Loop = Loop?\n");
    let error = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap_err();

    assert!(
        matches!(
            &error,
            TypeNormalizationError::Rule(violation)
                if violation.rule() == crate::TypeNormalizationRule::RecursiveAlias
        ),
        "{error:?}"
    );
}

#[test]
fn rejects_ambiguous_concrete_associated_projection() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut sources,
        concat!(
            "struct Buffer {}\n",
            "interface Source { pub type Item }\n",
            "conform Source for Buffer { type Item = i32 }\n",
            "conform Source for Buffer { type Item = i32 }\n",
            "func read(value: &Buffer): Buffer.Item { return 0 }\n",
        ),
    );
    let error = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        TypeNormalizationError::Rule(violation)
            if violation.rule() == crate::TypeNormalizationRule::AmbiguousAssociatedType
    ));
}

#[test]
fn normalization_is_non_recursive_for_deep_prefix_types() {
    let mut sources = SourceMap::new();
    let source = format!("type Deep = {}i32\n", "*".repeat(5_000));
    let (manifest, app, std_manifest, std_root, prelude) = fixture(&mut sources, &source);
    let normalized = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();

    assert!(
        normalized
            .type_for(all_nodes(&app, NodeKind::Type)[0])
            .is_some()
    );
}

#[test]
fn normalizes_opaque_result_identity_interface_bindings_and_outcomes() {
    let mut sources = SourceMap::new();
    let (manifest, app, std_manifest, std_root, prelude) = fixture(
        &mut sources,
        concat!(
            "interface Source<T> { pub type Item }\n",
            "func values<T>(): some Source<T, Item = &T>?! { return }\n",
        ),
    );
    let normalized = normalized_app(
        &sources,
        &manifest,
        &app,
        &std_manifest,
        &std_root,
        &prelude,
    )
    .unwrap();
    let store = normalized
        .namespaces()
        .imports
        .generics
        .headers
        .reserved
        .program
        .types();
    let result = normalized
        .callable_result(crate::SurfaceDeclarationId::from_index(2))
        .expect("function has a result");
    let Some(TypeKind::Fallible(optional)) = store.get(result) else {
        panic!("expected fallible opaque result");
    };
    let Some(TypeKind::Optional(opaque)) = store.get(*optional) else {
        panic!("expected optional opaque result");
    };
    let Some(TypeKind::Opaque {
        definition,
        arguments,
    }) = store.get(*opaque)
    else {
        panic!("expected opaque identity");
    };
    let contract = normalized
        .opaque_result(*definition)
        .expect("opaque result contract exists");

    assert_eq!(arguments.len(), 1);
    assert_eq!(contract.generic_parameters().len(), 1);
    assert_eq!(contract.interface().arguments(), arguments.as_ref());
    assert_eq!(contract.associated_types().len(), 1);
    assert_eq!(contract.result(), result);
    assert!(matches!(
        store.get(contract.associated_types()[0].ty()),
        Some(TypeKind::Borrow { .. })
    ));
}
