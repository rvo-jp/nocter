use nocter_model::Symbol;
use nocter_source::SourceMap;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};
pub(crate) use nocter_syntax::{
    child_nodes as direct_nodes, direct_identifier, direct_node as direct_child,
    direct_nodes as direct_children, first_direct_token as direct_token,
};

use crate::names::NameResolutionInternalError;

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
            nocter_syntax::SyntaxOrigin::Token(token),
        ))
}
