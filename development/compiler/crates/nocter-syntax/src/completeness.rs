use nocter_source::TextRange;

use crate::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

/// Reports whether one syntax subtree is structurally complete and diagnostic-free.
///
/// A source file may contain an incomplete body beside complete declarations. Consumers that can
/// safely retain those declarations use this exact syntax-owned fact instead of treating
/// [`SyntaxTree::has_errors`] as a file-wide admission decision.
#[must_use]
pub fn node_is_complete(tree: &SyntaxTree, root: NodeId) -> bool {
    let Some(root_node) = tree.node(root) else {
        return false;
    };
    let range = root_node.range();
    if tree
        .lexed()
        .diagnostics()
        .iter()
        .any(|diagnostic| range_reaches_diagnostic(range, diagnostic.span().range()))
        || tree
            .diagnostics()
            .iter()
            .any(|diagnostic| range_reaches_diagnostic(range, diagnostic.span().range()))
    {
        return false;
    }

    let mut pending = vec![root];
    while let Some(node_id) = pending.pop() {
        let Some(node) = tree.node(node_id) else {
            return false;
        };
        if node.kind() == NodeKind::Error {
            return false;
        }
        for element in tree.children(node_id) {
            match element {
                SyntaxElement::Node(child) => pending.push(*child),
                SyntaxElement::Missing(_) => return false,
                SyntaxElement::Token(_) => {}
            }
        }
    }
    true
}

const fn range_reaches_diagnostic(range: TextRange, diagnostic: TextRange) -> bool {
    if diagnostic.is_empty() {
        range.contains_cursor(diagnostic.start())
    } else {
        range.overlaps(diagnostic)
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};

    use super::*;
    use crate::{ParseGoal, descendant_node_iter, parse};

    #[test]
    fn distinguishes_complete_headers_from_an_incomplete_body() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("incomplete.nct"),
                b"#target: \"x64-linux\"\nfunc value(): void {\n    let missing =\n}\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(tree.has_errors());

        let target = descendant_node_iter(&tree, tree.root_id())
            .find(|node| tree.node(*node).unwrap().kind() == NodeKind::TargetDirective)
            .unwrap();
        let block = descendant_node_iter(&tree, tree.root_id())
            .find(|node| tree.node(*node).unwrap().kind() == NodeKind::Block)
            .unwrap();

        assert!(node_is_complete(&tree, target));
        assert!(!node_is_complete(&tree, block));
        assert!(!node_is_complete(&tree, tree.root_id()));
    }
}
