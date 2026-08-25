use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{NodeId, NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, UseResolutionInput, apply_toolchain_profile,
    collect_declaration_surface, prepare_authored_imports, prepare_declaration_headers,
    prepare_generic_binders, reserve_declaration_identities,
};

use super::{PreparedTypeBindings, TypeBindingError, bind_header_type_syntax};

pub(super) fn add_source(
    sources: &mut SourceMap,
    name: &str,
    text: &str,
) -> nocter_source::SourceId {
    sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap()
}

pub(super) fn parse_source(
    sources: &SourceMap,
    source: nocter_source::SourceId,
    goal: ParseGoal,
) -> SyntaxTree {
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors());
    tree
}

pub(super) fn package(
    identity: &str,
    display_name: &str,
    _path: &str,
    _manifest: &SyntaxTree,
) -> PackageInput {
    PackageInput::new(
        PackageIdentity::new(identity),
        display_name,
        PackageMode::Declared,
    )
}

pub(super) fn module<'syntax>(
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

pub(super) fn bind<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> Result<PreparedTypeBindings<'syntax>, TypeBindingError> {
    let builtin_source = modules
        .iter()
        .find(|module| {
            module.identity().package() == prelude.package() && module.identity().path().is_empty()
        })
        .and_then(|module| module.sources().first())
        .map(crate::ModuleSourceInput::syntax)
        .expect("test standard package has no root builtin source");
    let toolchain = crate::test_support::test_toolchain(prelude.clone(), builtin_source);
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        sources,
        packages,
        modules,
        uses,
    )
    .with_toolchain(toolchain.clone());
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_toolchain_profile(imports, &toolchain).unwrap();
    bind_header_type_syntax(namespaces)
}

pub(super) fn first_node(tree: &SyntaxTree, kind: NodeKind) -> NodeId {
    all_nodes(tree, kind)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {kind:?}"))
}

pub(super) fn all_nodes(tree: &SyntaxTree, kind: NodeKind) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|candidate| candidate.kind() == kind)
        {
            found.push(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    found
}
