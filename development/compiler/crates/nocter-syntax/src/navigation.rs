use crate::{NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};

/// Iterates direct child nodes in source order without allocating.
#[must_use]
pub fn child_node_iter(
    tree: &SyntaxTree,
    parent: NodeId,
) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
    tree.children(parent)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            _ => None,
        })
}

/// Collects direct child nodes in source order.
#[must_use]
pub fn child_nodes(tree: &SyntaxTree, parent: NodeId) -> Vec<NodeId> {
    child_node_iter(tree, parent).collect()
}

/// Iterates direct child nodes of one exact kind in source order without allocating.
#[must_use]
pub fn direct_node_iter(
    tree: &SyntaxTree,
    parent: NodeId,
    kind: NodeKind,
) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
    child_node_iter(tree, parent)
        .filter(move |child| tree.node(*child).is_some_and(|node| node.kind() == kind))
}

/// Collects direct child nodes of one exact kind in source order.
#[must_use]
pub fn direct_nodes(tree: &SyntaxTree, parent: NodeId, kind: NodeKind) -> Vec<NodeId> {
    direct_node_iter(tree, parent, kind).collect()
}

/// Returns the first direct child node of one exact kind.
#[must_use]
pub fn direct_node(tree: &SyntaxTree, parent: NodeId, kind: NodeKind) -> Option<NodeId> {
    direct_node_iter(tree, parent, kind).next()
}

/// Iterates direct child tokens in source order without allocating.
#[must_use]
pub fn direct_token_iter(
    tree: &SyntaxTree,
    parent: NodeId,
) -> impl DoubleEndedIterator<Item = SyntaxToken> + '_ {
    tree.children(parent)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
}

/// Iterates direct identifier tokens in source order without allocating.
#[must_use]
pub fn direct_identifier_iter(
    tree: &SyntaxTree,
    parent: NodeId,
) -> impl DoubleEndedIterator<Item = SyntaxToken> + '_ {
    direct_token_iter(tree, parent).filter(|token| token.kind() == TokenKind::Identifier)
}

/// Collects direct child tokens in source order.
#[must_use]
pub fn direct_tokens(tree: &SyntaxTree, parent: NodeId) -> Vec<SyntaxToken> {
    direct_token_iter(tree, parent).collect()
}

/// Returns the first direct child token.
#[must_use]
pub fn first_direct_token(tree: &SyntaxTree, parent: NodeId) -> Option<SyntaxToken> {
    direct_token_iter(tree, parent).next()
}

/// Returns the first direct child token of one exact kind.
#[must_use]
pub fn direct_token(tree: &SyntaxTree, parent: NodeId, kind: TokenKind) -> Option<SyntaxToken> {
    tree.children(parent)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == kind => Some(*token),
            _ => None,
        })
}

/// Returns the first direct identifier token.
#[must_use]
pub fn direct_identifier(tree: &SyntaxTree, parent: NodeId) -> Option<SyntaxToken> {
    direct_identifier_iter(tree, parent).next()
}

/// Iterates every descendant token in source order without allocating output storage.
pub fn descendant_token_iter(
    tree: &SyntaxTree,
    parent: NodeId,
) -> impl Iterator<Item = SyntaxToken> + '_ {
    let mut pending: Vec<_> = tree.children(parent).iter().rev().copied().collect();
    std::iter::from_fn(move || {
        loop {
            match pending.pop()? {
                SyntaxElement::Node(node) => {
                    pending.extend(tree.children(node).iter().rev().copied());
                }
                SyntaxElement::Token(token) => return Some(token),
                SyntaxElement::Missing(_) => {}
            }
        }
    })
}

/// Iterates every descendant identifier token in source order without allocating output storage.
pub fn descendant_identifier_iter(
    tree: &SyntaxTree,
    parent: NodeId,
) -> impl Iterator<Item = SyntaxToken> + '_ {
    descendant_token_iter(tree, parent).filter(|token| token.kind() == TokenKind::Identifier)
}

/// Iterates outermost descendants of one kind in source order.
///
/// Once a matching node is yielded, traversal does not enter that node. The pruning rule prevents
/// a query for declaration-like containers from also returning nested containers of the same kind.
pub fn outermost_descendant_node_iter(
    tree: &SyntaxTree,
    parent: NodeId,
    kind: NodeKind,
) -> impl Iterator<Item = NodeId> + '_ {
    let mut pending: Vec<_> = tree.children(parent).iter().rev().copied().collect();
    std::iter::from_fn(move || {
        loop {
            let SyntaxElement::Node(node) = pending.pop()? else {
                continue;
            };
            if tree
                .node(node)
                .is_some_and(|candidate| candidate.kind() == kind)
            {
                return Some(node);
            }
            pending.extend(tree.children(node).iter().rev().copied());
        }
    })
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};

    use crate::{NodeKind, ParseGoal, parse};

    use super::{
        descendant_identifier_iter, direct_identifier_iter, outermost_descendant_node_iter,
    };

    #[test]
    fn shared_navigation_preserves_source_order_and_explicit_pruning() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("navigation.nct"),
                b"func main(value: i32): void { if true { let copy = value } }\n",
            )
            .unwrap();
        let source = sources.get(source).unwrap();
        let tree = parse(source, ParseGoal::SourceFile);
        assert!(!tree.has_errors());
        let function = tree
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            .unwrap()
            .0;

        let direct = direct_identifier_iter(&tree, function)
            .map(|token| source.text_at(token.range()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(direct, ["main"]);

        let descendants = descendant_identifier_iter(&tree, function)
            .map(|token| source.text_at(token.range()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(descendants, ["main", "value", "i32", "copy", "value"]);

        let blocks =
            outermost_descendant_node_iter(&tree, function, NodeKind::Block).collect::<Vec<_>>();
        assert_eq!(blocks.len(), 1);
        assert!(
            tree.children(blocks[0])
                .iter()
                .any(|child| matches!(child, crate::SyntaxElement::Node(_)))
        );
    }
}
