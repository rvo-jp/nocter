use crate::{Keyword, NodeId, NodeKind, Punctuation, SyntaxToken, SyntaxTree, TokenKind};

/// Returns the declaration-name token defined by the grammar for one declaration node.
///
/// This is the shared syntactic authority for discovery locators, semantic lowering, and source
/// presentation. Callers must not recover declaration names by choosing an arbitrary descendant
/// identifier.
#[must_use]
pub fn declaration_name_token(tree: &SyntaxTree, declaration: NodeId) -> Option<SyntaxToken> {
    let kind = tree.node(declaration)?.kind();
    let tokens = descendant_tokens(tree, declaration);
    match kind {
        NodeKind::ConstantDeclaration => identifier_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Const)
        }),
        NodeKind::FunctionDeclaration | NodeKind::ConstructionFunction => {
            identifier_after(&tokens, |token| {
                token.kind() == TokenKind::Keyword(Keyword::Func)
            })
        }
        NodeKind::PrimitiveTypeDeclaration => name_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Type)
        }),
        NodeKind::TypeAliasDeclaration | NodeKind::AssociatedTypeDeclaration => {
            identifier_after(&tokens, |token| {
                token.kind() == TokenKind::Keyword(Keyword::Type)
            })
        }
        NodeKind::StructDeclaration => identifier_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Struct)
        }),
        NodeKind::EnumDeclaration => identifier_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Enum)
        }),
        NodeKind::InterfaceDeclaration => identifier_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Interface)
        }),
        NodeKind::TestDeclaration => identifier_after(&tokens, |token| {
            token.kind() == TokenKind::Keyword(Keyword::Test)
        }),
        NodeKind::InterfaceMethod | NodeKind::InherentMethod => {
            identifier_after(&tokens, |token| {
                token.kind() == TokenKind::Punctuation(Punctuation::Dot)
            })
        }
        NodeKind::StructField | NodeKind::EnumVariant => tokens
            .iter()
            .copied()
            .find(|token| token.kind() == TokenKind::Identifier),
        _ => None,
    }
}

fn identifier_after(
    tokens: &[SyntaxToken],
    mut predicate: impl FnMut(SyntaxToken) -> bool,
) -> Option<SyntaxToken> {
    let marker = tokens.iter().position(|token| predicate(*token))?;
    tokens[marker + 1..]
        .iter()
        .copied()
        .find(|token| token.kind() == TokenKind::Identifier)
}

fn name_after(
    tokens: &[SyntaxToken],
    mut predicate: impl FnMut(SyntaxToken) -> bool,
) -> Option<SyntaxToken> {
    let marker = tokens.iter().position(|token| predicate(*token))?;
    tokens[marker + 1..].iter().copied().find(|token| {
        matches!(
            token.kind(),
            TokenKind::Identifier | TokenKind::Keyword(Keyword::Void | Keyword::Never)
        )
    })
}

fn descendant_tokens(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    crate::descendant_token_iter(tree, node).collect()
}
