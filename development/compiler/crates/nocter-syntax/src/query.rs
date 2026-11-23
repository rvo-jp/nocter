use crate::{Keyword, NodeId, NodeKind, Punctuation, SyntaxToken, SyntaxTree, TokenKind};

/// Select the seed and count expression roots of one syntactic array repetition.
/// This proves structural roles only, without completeness, type or evaluation evidence.
#[must_use]
pub fn array_repetition_parts(tree: &SyntaxTree, array: NodeId) -> Option<(NodeId, NodeId)> {
    if tree.node(array)?.kind() != NodeKind::ArrayRepeatLiteral {
        return None;
    }
    let mut expressions = crate::direct_node_iter(tree, array, NodeKind::Expression);
    let seed = expressions.next()?;
    let count = expressions.next()?;
    expressions.next().is_none().then_some((seed, count))
}

/// Returns only the implementation root directly owned by a declaration.
/// Blocks nested in header expressions never become callable implementations.
#[must_use]
pub fn declaration_body(tree: &SyntaxTree, declaration: NodeId) -> Option<NodeId> {
    let body = match tree.node(declaration)?.kind() {
        NodeKind::FunctionDeclaration
        | NodeKind::ConstructionFunction
        | NodeKind::LiteralDeclaration
        | NodeKind::InherentMethod
        | NodeKind::InterfaceMethod
        | NodeKind::CoercionDeclaration
        | NodeKind::EqualityOperator
        | NodeKind::OrderingOperator
        | NodeKind::IndexOperator
        | NodeKind::ExpansionOperator
        | NodeKind::DropDeclaration
        | NodeKind::TestDeclaration => NodeKind::Block,
        NodeKind::ConstantDeclaration | NodeKind::StaticDeclaration => NodeKind::Expression,
        _ => return None,
    };
    crate::direct_node(tree, declaration, body)
}

/// Returns the authored owner name and its pattern for a v2 nominal behavior fragment.
#[must_use]
pub fn type_behavior_owner(
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Option<(NodeId, SyntaxToken)> {
    if tree.node(declaration)?.kind() != NodeKind::TypeBehaviorDeclaration {
        return None;
    }
    let pattern = crate::direct_node(tree, declaration, NodeKind::DeclarationTypePattern)?;
    Some((pattern, crate::direct_identifier(tree, pattern)?))
}

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
        NodeKind::StaticDeclaration => {
            let introducer = declaration_contextual_keyword_token(tree, declaration)?;
            identifier_after(&tokens, |token| token == introducer)
        }
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
        NodeKind::StructField | NodeKind::EnumVariant => {
            crate::direct_token_iter(tree, declaration)
                .find(|token| token.kind() == TokenKind::Identifier)
        }
        _ => None,
    }
}

/// Returns the exact contextual-keyword token owned by a declaration grammar production.
///
/// Contextual words remain identifiers in the lexer so they can still be ordinary names outside
/// their grammar position. Structural tooling must query the parser-owned role instead of
/// duplicating source-text tests.
#[must_use]
pub fn declaration_contextual_keyword_token(
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Option<SyntaxToken> {
    match tree.node(declaration)?.kind() {
        NodeKind::StaticDeclaration | NodeKind::CoercionDeclaration => {
            tree.children(declaration).iter().find_map(|element| {
                let crate::SyntaxElement::Token(token) = element else {
                    return None;
                };
                (token.kind() == TokenKind::Identifier).then_some(*token)
            })
        }
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

/// Select exactly one declaration from an item, ignoring only its directive prefix nodes.
/// This grants structural identity only, not directive validity or declaration acceptance.
#[must_use]
pub fn item_declaration(tree: &SyntaxTree, item: NodeId) -> Option<NodeId> {
    if tree.node(item)?.kind() != NodeKind::Item {
        return None;
    }
    let mut children = crate::child_node_iter(tree, item).filter(|node| {
        !matches!(
            tree.node(*node).map(crate::SyntaxNode::kind),
            Some(NodeKind::Visibility | NodeKind::TargetDirective)
        )
    });
    let declaration = children.next()?;
    (children.next().is_none() && is_declaration(tree.node(declaration)?.kind()))
        .then_some(declaration)
}

pub(crate) const fn is_declaration(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::ConstantDeclaration
            | NodeKind::StaticDeclaration
            | NodeKind::PrimitiveTypeDeclaration
            | NodeKind::TypeAliasDeclaration
            | NodeKind::StructDeclaration
            | NodeKind::StructField
            | NodeKind::EnumDeclaration
            | NodeKind::EnumVariant
            | NodeKind::InterfaceDeclaration
            | NodeKind::AssociatedTypeDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructDeclaration
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InstanceDeclaration
            | NodeKind::TypeBehaviorDeclaration
            | NodeKind::StructuralLiteralGrant
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::InterfaceImplementation
            | NodeKind::AssociatedTypeBinding
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
    )
}
