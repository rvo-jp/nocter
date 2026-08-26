use nocter_model::BuiltinType;
use nocter_syntax::{
    NodeId, NodeKind, SyntaxElement, SyntaxTree, child_nodes, declaration_name_token,
};

use crate::{
    ModuleIdentity, PackageTargetResolutionInput, SourceVisibilityResolutionInput, ToolchainInput,
    UseResolutionInput,
};

pub(crate) const TEST_BUILTIN_SOURCE: &str = "\
pub primitive type bool\n\
pub primitive type i8\n\
pub primitive type i16\n\
pub primitive type i32\n\
pub primitive type i64\n\
pub primitive type u8\n\
pub primitive type u16\n\
pub primitive type u32\n\
pub primitive type u64\n\
pub primitive type usize\n\
pub primitive type isize\n\
pub primitive type str\n\
pub primitive type error\n\
pub primitive type void\n\
pub primitive type never\n";

pub(crate) fn test_toolchain(
    prelude: ModuleIdentity,
    builtin_module: &ModuleIdentity,
    builtin_source: &SyntaxTree,
) -> ToolchainInput {
    let mut declarations = Vec::new();
    let mut pending = vec![builtin_source.root_id()];
    while let Some(node) = pending.pop() {
        if builtin_source
            .node(node)
            .is_some_and(|node| node.kind() == NodeKind::PrimitiveTypeDeclaration)
        {
            declarations.push(
                declaration_name_token(builtin_source, node)
                    .expect("test primitive type has no declaration name"),
            );
        }
        pending.extend(child_nodes(builtin_source, node).into_iter().rev());
    }
    assert_eq!(declarations.len(), BuiltinType::COUNT);
    let builtin_types = BuiltinType::ALL
        .iter()
        .copied()
        .map(|builtin| {
            nocter_compile_input::BuiltinTypeLocator::new(
                builtin,
                builtin_module.clone(),
                builtin.spelling(),
            )
        })
        .collect();
    ToolchainInput::new(prelude.package().clone(), prelude, Vec::new(), Vec::new())
        .with_builtin_types(builtin_types)
}

pub(crate) fn empty_toolchain(module: ModuleIdentity) -> ToolchainInput {
    ToolchainInput::new(module.package().clone(), module, Vec::new(), Vec::new())
}

pub(crate) fn source_see(
    tree: &SyntaxTree,
    position: usize,
    canonical_target: &str,
) -> SourceVisibilityResolutionInput {
    SourceVisibilityResolutionInput::new(
        source_visibility_declarations(tree)[position],
        canonical_target,
    )
}

pub(crate) fn module_use(
    tree: &SyntaxTree,
    position: usize,
    target: ModuleIdentity,
) -> UseResolutionInput {
    UseResolutionInput::new(use_declarations(tree)[position], target)
}

fn source_visibility_declarations(tree: &SyntaxTree) -> Vec<NodeId> {
    tree.children(tree.root_id())
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node)
                if tree
                    .node(*node)
                    .is_some_and(|node| node.kind() == NodeKind::SourceVisibilityDeclaration) =>
            {
                Some(*node)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .collect()
}

pub(crate) fn package_target(
    sources: &nocter_source::SourceMap,
    tree: &SyntaxTree,
    position: usize,
    module: ModuleIdentity,
) -> PackageTargetResolutionInput {
    let source = sources.get(tree.source()).unwrap();
    let declaration = nocter_package::decode_package_declaration(source, tree).unwrap();
    let target = &declaration.targets()[position];
    PackageTargetResolutionInput::new(
        target.declaration(),
        target.name().value(),
        target.name().literal(),
        target.kind(),
        target.order(),
        module,
    )
}

fn use_declarations(tree: &SyntaxTree) -> Vec<NodeId> {
    let mut declarations = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| {
            matches!(
                node.kind(),
                NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
            )
        }) {
            declarations.push(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    declarations
}
