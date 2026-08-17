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
    PackageTargetResolutionInput::new(package_target_declarations(sources, tree)[position], module)
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

fn package_target_declarations(
    sources: &nocter_source::SourceMap,
    tree: &SyntaxTree,
) -> Vec<NodeId> {
    let source = sources.get(tree.source()).unwrap();
    tree.children(tree.root_id())
        .iter()
        .filter_map(|element| {
            let SyntaxElement::Node(node) = element else {
                return None;
            };
            tree.children(*node)
                .iter()
                .any(|element| match element {
                    SyntaxElement::Token(token) => {
                        token.kind()
                            == nocter_syntax::TokenKind::Keyword(nocter_syntax::Keyword::Test)
                            || (token.kind() == nocter_syntax::TokenKind::Identifier
                                && source.text_at(token.range()) == Some("executable"))
                    }
                    SyntaxElement::Node(_) | SyntaxElement::Missing(_) => false,
                })
                .then_some(*node)
        })
        .collect()
}
