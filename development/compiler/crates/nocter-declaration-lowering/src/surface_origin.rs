use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxToken, SyntaxTree, TokenKind, direct_node,
    direct_token,
};

use crate::SurfaceDeclarationKind;

/// Selects the exact syntax that carries a declaration's semantic identity.
///
/// Named declarations use their name. Unnamed callable surfaces use the operator, coercion, or
/// literal shape that distinguishes the declaration. Structural owner declarations retain their
/// complete node for diagnostic projection but are not themselves interactive editor subjects.
pub(crate) fn declaration_entity_origin(
    tree: &SyntaxTree,
    node: NodeId,
    kind: SurfaceDeclarationKind,
    name: Option<SyntaxToken>,
) -> Option<SyntaxOrigin> {
    if let Some(name) = name {
        return Some(SyntaxOrigin::Token(name));
    }
    let token_kind = match kind {
        SurfaceDeclarationKind::Coercion => Some(TokenKind::Keyword(Keyword::As)),
        SurfaceDeclarationKind::Equality => Some(TokenKind::Punctuation(Punctuation::EqualEqual)),
        SurfaceDeclarationKind::Ordering => Some(TokenKind::Punctuation(Punctuation::Less)),
        SurfaceDeclarationKind::Index => Some(TokenKind::Punctuation(Punctuation::LeftBracket)),
        SurfaceDeclarationKind::Expansion => Some(TokenKind::Punctuation(Punctuation::Expansion)),
        SurfaceDeclarationKind::OpaqueType => Some(TokenKind::Identifier),
        SurfaceDeclarationKind::Literal => {
            return direct_node(tree, node, NodeKind::LiteralShape).map(SyntaxOrigin::Node);
        }
        _ => None,
    };
    match token_kind {
        Some(token_kind) => direct_token(tree, node, token_kind).map(SyntaxOrigin::Token),
        None => Some(SyntaxOrigin::Node(node)),
    }
}
