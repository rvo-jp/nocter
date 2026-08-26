use nocter_source::SourceFile;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree, direct_node};

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
            let path = use_path_node(tree, node)
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

pub(crate) fn use_path_node(tree: &SyntaxTree, declaration: NodeId) -> Option<NodeId> {
    direct_node(tree, declaration, NodeKind::ModulePath)
}
