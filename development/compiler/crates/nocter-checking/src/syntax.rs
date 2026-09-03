use nocter_model::Symbol;
use nocter_source::SourceMap;
use nocter_syntax::{NodeId, NodeKind, SyntaxToken, SyntaxTree};
pub(crate) use nocter_syntax::{
    child_nodes, direct_identifier, direct_node, direct_nodes, direct_token, first_direct_token,
};

use crate::names::NameResolutionInternalError;

pub(crate) fn outermost_descendants(
    tree: &SyntaxTree,
    root: NodeId,
    expected: NodeKind,
) -> Vec<NodeId> {
    nocter_syntax::outermost_descendant_node_iter(tree, root, expected).collect()
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

pub(crate) fn descendant_identifiers(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    nocter_syntax::descendant_identifier_iter(tree, node).collect()
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
