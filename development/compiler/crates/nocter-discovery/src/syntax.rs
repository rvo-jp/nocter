use nocter_source::SourceFile;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::DiscoveryError;

pub(crate) fn active_use_paths(
    source: &SourceFile,
    tree: &SyntaxTree,
    active: &nocter_target_selection::TargetSelection,
) -> Result<Vec<(NodeId, Box<str>)>, DiscoveryError> {
    let mut result = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        let kind = tree
            .node(node)
            .ok_or(DiscoveryError::InconsistentSyntax(node))?
            .kind();
        if matches!(
            kind,
            NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
        ) && active.use_is_active(node)
        {
            let path = direct_child(tree, node, NodeKind::ModulePath)
                .and_then(|path| tree.node(path))
                .and_then(|path| source.text_at(path.range()))
                .ok_or(DiscoveryError::InconsistentSyntax(node))?;
            result.push((node, path.into()));
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    result.sort_unstable_by_key(|(node, _)| tree.node(*node).map(|node| node.range().start()));
    Ok(result)
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
