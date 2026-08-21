use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::{ModuleIdentity, PackageTargetResolutionInput, UseResolutionInput, UseTargetInput};

pub(crate) fn source_use(
    tree: &SyntaxTree,
    position: usize,
    canonical_target: &str,
) -> UseResolutionInput {
    UseResolutionInput::new(
        use_declarations(tree)[position],
        UseTargetInput::Source(canonical_target.into()),
    )
}

pub(crate) fn module_use(
    tree: &SyntaxTree,
    position: usize,
    target: ModuleIdentity,
) -> UseResolutionInput {
    UseResolutionInput::new(
        use_declarations(tree)[position],
        UseTargetInput::Module(target),
    )
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
