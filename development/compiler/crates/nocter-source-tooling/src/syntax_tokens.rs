use std::collections::HashSet;

use nocter_syntax::{SyntaxElement, SyntaxToken, SyntaxTree};

/// Returns each CST token exactly once in normalized source order.
///
/// Parser-owned token subdivisions remain distinct even when they share one lexical token.
pub(super) fn ordered(syntax: &SyntaxTree) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let mut seen = HashSet::new();
    for (node_id, _) in syntax.nodes() {
        for child in syntax.children(node_id) {
            if let SyntaxElement::Token(token) = child
                && seen.insert(*token)
            {
                tokens.push(*token);
            }
        }
    }
    tokens.sort_by_key(|token| {
        (
            token.range().start(),
            token.range().end(),
            token.lexical().index(),
            token.kind().as_str(),
        )
    });
    tokens
}
