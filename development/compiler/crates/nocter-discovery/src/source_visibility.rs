use nocter_source::SourceFile;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::DiscoveryError;

/// Extracts exact top-level see paths without traversing item or block syntax.
pub(crate) fn source_visibility_paths(
    source: &SourceFile,
    tree: &SyntaxTree,
) -> Result<Vec<(NodeId, Box<str>)>, DiscoveryError> {
    let mut result = Vec::new();
    for element in tree.children(tree.root_id()) {
        let SyntaxElement::Node(declaration) = element else {
            continue;
        };
        if tree.node(*declaration).map(nocter_syntax::SyntaxNode::kind)
            != Some(NodeKind::SourceVisibilityDeclaration)
        {
            continue;
        }
        let path = direct_child(tree, *declaration, NodeKind::SourceVisibilityPath)
            .and_then(|path| tree.node(path))
            .and_then(|path| source.text_at(path.range()))
            .ok_or(DiscoveryError::InconsistentSyntax(*declaration))?;
        result.push((*declaration, path.into()));
    }
    Ok(result)
}

pub(crate) fn source_visibility_path_node(
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Option<NodeId> {
    direct_child(tree, declaration, NodeKind::SourceVisibilityPath)
}

fn direct_child(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree.node(*child).is_some_and(|node| node.kind() == kind) =>
            {
                Some(*child)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}
