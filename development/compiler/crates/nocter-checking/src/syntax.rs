use nocter_model::Symbol;
use nocter_source::SourceMap;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};

use crate::names::NameResolutionInternalError;

pub(crate) fn direct_child(tree: &SyntaxTree, node: NodeId, expected: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|node| node.kind() == expected) =>
            {
                Some(*child)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

pub(crate) fn direct_children(tree: &SyntaxTree, node: NodeId, expected: NodeKind) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|node| node.kind() == expected) =>
            {
                Some(*child)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .collect()
}

pub(crate) fn direct_nodes(tree: &SyntaxTree, node: NodeId) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(child) => Some(*child),
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .collect()
}

pub(crate) fn descendants(tree: &SyntaxTree, root: NodeId, expected: NodeKind) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node != root && tree.node(node).is_some_and(|node| node.kind() == expected) {
            found.push(node);
            continue;
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    found
}

pub(crate) fn direct_token(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })
}

pub(crate) fn is_transparent_expression(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Expression
            | NodeKind::LogicalOrExpression
            | NodeKind::LogicalAndExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression
            | NodeKind::ConversionExpression
            | NodeKind::GroupedExpression
    )
}

pub(crate) fn direct_identifier(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => Some(*token),
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

pub(crate) fn identifier_tokens(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let mut pending: Vec<_> = tree.children(node).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(child) => {
                pending.extend(tree.children(child).iter().rev().copied());
            }
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => {
                tokens.push(token);
            }
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
        }
    }
    tokens
}

pub(crate) fn token_symbol(
    sources: &SourceMap,
    symbols: &nocter_model::SymbolTable,
    token: SyntaxToken,
) -> Result<Symbol, NameResolutionInternalError> {
    let spelling = token_text(sources, token)?;
    symbols
        .get(spelling)
        .ok_or_else(|| NameResolutionInternalError::MissingSymbol(spelling.into()))
}

pub(crate) fn token_text(
    sources: &SourceMap,
    token: SyntaxToken,
) -> Result<&str, NameResolutionInternalError> {
    sources
        .get(token.source())
        .and_then(|source| source.text_at(token.range()))
        .ok_or(NameResolutionInternalError::InvalidSyntaxOrigin(
            nocter_source_index::SyntaxOrigin::Token(token),
        ))
}
