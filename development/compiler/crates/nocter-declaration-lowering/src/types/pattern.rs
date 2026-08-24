use nocter_declarations::ExportedEntity;
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::context::{builtin_type, declaration_source, require_arity, token_symbol};
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
    if let Some(builtin) = direct_node(tree, pattern, NodeKind::BuiltinType) {
        return Ok(BoundDeclarationPattern::Builtin(builtin_type(
            namespaces, tree, builtin,
        )?));
    }
    if has_punctuation(tree, pattern, Punctuation::LeftBracket) {
        let token =
            direct_identifier(tree, pattern).ok_or(TypeBindingError::InvalidSyntax(pattern))?;
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

    let head = direct_identifier(tree, pattern).ok_or(TypeBindingError::InvalidSyntax(pattern))?;
    let name = token_symbol(namespaces, tree, head)?;
    let source = declaration_source(namespaces, declaration)?;
    let entity = namespaces
        .lookup_local(source, name)
        .ok_or(TypeBindingError::rule(
            TypeBindingRule::UnknownTypeContextName,
            SyntaxOrigin::Token(head),
        ))?;
    projection::reference(namespaces, tree, entity, head)?;
    let arguments_node = direct_node(tree, pattern, NodeKind::PatternArguments);
    let arguments = arguments_node
        .map(|arguments| bind_arguments(namespaces, declaration, tree, arguments))
        .transpose()?
        .unwrap_or_default();
    match entity {
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
    identifier_tokens(tree, arguments)
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

fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree.node(*child).is_some_and(|node| node.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
}

fn direct_nodes(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(child)
                if tree.node(*child).is_some_and(|node| node.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
        .collect()
}

fn direct_identifier(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => Some(*token),
            _ => None,
        })
}

fn identifier_tokens(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => Some(*token),
            _ => None,
        })
        .collect()
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
