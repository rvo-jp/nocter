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
    direct_token(tree, parent, TokenKind::Identifier)
}
