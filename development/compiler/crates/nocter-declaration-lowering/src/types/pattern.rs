use nocter_declarations::ExportedEntity;
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind, direct_node,
    direct_nodes,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::context::{declaration_source, require_arity, token_symbol};
use super::{BoundDeclarationPattern, TypeBindingError, TypeBindingRule, projection};

pub(super) fn bind_all(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    root: NodeId,
) -> Result<Vec<BoundDeclarationPattern>, TypeBindingError> {
    direct_nodes(tree, root, NodeKind::DeclarationTypePattern)
        .into_iter()
        .map(|pattern| bind(namespaces, declaration, tree, pattern))
        .collect()
}

fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    pattern: NodeId,
) -> Result<BoundDeclarationPattern, TypeBindingError> {
    if has_punctuation(tree, pattern, Punctuation::LeftBracket) {
        let token =
            direct_type_name(tree, pattern).ok_or(TypeBindingError::InvalidSyntax(pattern))?;
        let name = token_symbol(namespaces, tree, token)?;
        let parameter = namespaces
            .imports
            .generics
            .lookup(declaration, name)
            .ok_or(TypeBindingError::rule(
                TypeBindingRule::UnknownTypeContextName,
                SyntaxOrigin::Token(token),
            ))?;
        return Ok(BoundDeclarationPattern::Slice(parameter));
    }

    let named = direct_node(tree, pattern, NodeKind::NamedType);
    let head = direct_type_name(tree, named.unwrap_or(pattern))
        .ok_or(TypeBindingError::InvalidSyntax(pattern))?;
    let name = token_symbol(namespaces, tree, head)?;
    let source = declaration_source(namespaces, declaration)?;
    let entity = namespaces
        .lookup_local(source, name)
        .ok_or(TypeBindingError::rule(
            TypeBindingRule::UnknownTypeContextName,
            SyntaxOrigin::Token(head),
        ))?;
    projection::reference(namespaces, tree, entity, head)?;
    let arguments_node = if named.is_some() {
        None
    } else {
        direct_node(tree, pattern, NodeKind::PatternArguments)
    };
    let arguments = arguments_node
        .map(|arguments| bind_arguments(namespaces, declaration, tree, arguments))
        .transpose()?
        .unwrap_or_default();
    match entity {
        ExportedEntity::BuiltinType(builtin) => {
            if !arguments.is_empty() {
                return Err(TypeBindingError::rule(
                    TypeBindingRule::InvalidTypeArguments,
                    arguments_node.map_or(SyntaxOrigin::Token(head), SyntaxOrigin::Node),
                ));
            }
            Ok(BoundDeclarationPattern::Builtin(builtin))
        }
        ExportedEntity::NominalType(definition) => {
            require_arity(
                namespaces,
                arguments_node.map_or(SyntaxOrigin::Token(head), SyntaxOrigin::Node),
                ReservedEntity::NominalType(definition),
                arguments.len(),
            )?;
            Ok(BoundDeclarationPattern::Nominal {
                definition,
                arguments: arguments.into_boxed_slice(),
            })
        }
        ExportedEntity::Interface(definition) => {
            require_arity(
                namespaces,
                arguments_node.map_or(SyntaxOrigin::Token(head), SyntaxOrigin::Node),
                ReservedEntity::Interface(definition),
                arguments.len(),
            )?;
            Ok(BoundDeclarationPattern::Interface {
                definition,
                arguments: arguments.into_boxed_slice(),
            })
        }
        ExportedEntity::Module(_)
        | ExportedEntity::TypeAlias(_)
        | ExportedEntity::Constant(_)
        | ExportedEntity::Static(_)
        | ExportedEntity::Callable(_) => Err(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeEntity,
            SyntaxOrigin::Token(head),
        )),
    }
}

fn bind_arguments(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    arguments: NodeId,
) -> Result<Vec<nocter_model::GenericParameterId>, TypeBindingError> {
    direct_identifiers(tree, arguments)
        .into_iter()
        .map(|token| {
            let name = token_symbol(namespaces, tree, token)?;
            namespaces
                .imports
                .generics
                .lookup(declaration, name)
                .ok_or(TypeBindingError::rule(
                    TypeBindingRule::UnknownTypeContextName,
                    SyntaxOrigin::Token(token),
                ))
        })
        .collect()
}

fn direct_type_name(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if is_type_name(token.kind()) => Some(*token),
            _ => None,
        })
}

const fn is_type_name(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::Keyword(nocter_syntax::Keyword::Void | nocter_syntax::Keyword::Never)
    )
}

fn direct_identifiers(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    nocter_syntax::direct_identifier_iter(tree, node).collect()
}

fn has_punctuation(tree: &SyntaxTree, node: NodeId, punctuation: Punctuation) -> bool {
    tree.children(node).iter().any(|element| {
        matches!(
            element,
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    TokenKind::Punctuation(candidate) if candidate == punctuation
                )
        )
    })
}
