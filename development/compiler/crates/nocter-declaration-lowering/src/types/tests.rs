use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{NodeId, NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

use super::{BoundTypeKind, TypeBindingError, bind_header_type_syntax};
use crate::test_support::module_use;
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, UseResolutionInput,
    apply_standard_prelude, collect_declaration_surface, prepare_authored_imports,
    prepare_declaration_headers, prepare_generic_binders, reserve_declaration_identities,
};

fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
    sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap()
}

fn parse_source(
    sources: &SourceMap,
    source: nocter_source::SourceId,
    goal: ParseGoal,
) -> SyntaxTree {
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors());
    tree
}

fn package<'syntax>(
    identity: &str,
    display_name: &str,
    path: &str,
    manifest: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        PackageIdentity::new(identity),
        display_name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, manifest)),
    )
}

fn module<'syntax>(
    package: &str,
    path: &[&str],
    source_path: &str,
    syntax: &'syntax SyntaxTree,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied()),
        vec![ModuleSourceInput::new(
            source_path,
            ModuleSourceKind::Root,
            syntax,
        )],
    )
}

fn bind<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput<'syntax>>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> Result<super::PreparedTypeBindings<'syntax>, TypeBindingError> {
    let input = CompileUnitInput::new(sources, packages, modules, uses);
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_standard_prelude(imports, prelude).unwrap();
    bind_header_type_syntax(namespaces)
}

fn first_node(tree: &SyntaxTree, kind: NodeKind) -> NodeId {
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|candidate| candidate.kind() == kind)
        {
            return node;
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    panic!("missing {kind:?}");
}

#[test]
fn binds_qualified_generic_and_associated_type_shapes_before_normalization() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep\ntype Projection<T> = &dep.Buffer<T>.Item?!\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Buffer<T> {}\n");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let dep = parse_source(&sources, dep_id, ParseGoal::ModuleSource);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app),
            module("resolved:dep", &[], "/dep/index.nct", &dep),
            module("builtin:std", &[], "/std/index.nct", &std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude,
            ),
        ],
        vec![module_use(&app, 0, dep_identity)],
        &prelude_identity,
    )
    .unwrap();
    let root = bound.type_for(first_node(&app, NodeKind::Type)).unwrap();
    let BoundTypeKind::Fallible(optional) = bound.kind(root).unwrap() else {
        panic!("expected outer fallible type");
    };
    let BoundTypeKind::Optional(borrowed) = bound.kind(*optional).unwrap() else {
        panic!("expected optional success payload");
    };
    let BoundTypeKind::Borrow {
        capability: BorrowCapability::Readonly,
        referent,
    } = bound.kind(*borrowed).unwrap()
    else {
        panic!("expected readonly borrow");
    };
    let BoundTypeKind::AssociatedSelection { base, .. } = bound.kind(*referent).unwrap() else {
        panic!("expected unresolved associated selection");
    };
    let BoundTypeKind::Nominal { arguments, .. } = bound.kind(*base).unwrap() else {
        panic!("expected qualified nominal base");
    };
    assert!(matches!(
        bound.kind(arguments[0]),
        Some(BoundTypeKind::GenericParameter(_))
    ));
}

#[test]
fn normalizes_explicit_callable_origins_to_parameter_positions() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "type Callback<T> = &+func(left: &T, right: &T): &T from right | left\n",
    );
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app),
            module("builtin:std", &[], "/std/index.nct", &std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude,
            ),
        ],
        vec![],
        &prelude_identity,
    )
    .unwrap();
    let root = bound.type_for(first_node(&app, NodeKind::Type)).unwrap();
    let BoundTypeKind::Callable(callable) = bound.kind(root).unwrap() else {
        panic!("expected callable type");
    };

    assert_eq!(callable.capability(), CallableCapability::ReadWrite);
    assert_eq!(callable.parameters().len(), 2);
    assert_eq!(
        callable.explicit_origins(),
        Some([ParameterOrigin::new(0), ParameterOrigin::new(1)].as_slice())
    );
}
