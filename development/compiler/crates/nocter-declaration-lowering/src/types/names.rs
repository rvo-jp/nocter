use std::collections::HashMap;

use nocter_declarations::ExportedEntity;
use nocter_model::Symbol;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};

use crate::{PreparedNamespaces, SurfaceDeclarationId};

use super::context::{declaration_module, token_symbol};
use super::{BoundTypeId, TypeBindingError, projection};

pub(super) struct NameSegment {
    pub(super) token: SyntaxToken,
    pub(super) arguments: Vec<BoundTypeId>,
}

pub(super) struct TrailingSelection {
    pub(super) name: Symbol,
    pub(super) arguments: Vec<BoundTypeId>,
}

pub(super) struct ResolvedEntityPath {
    pub(super) entity: ExportedEntity,
    pub(super) arguments: Vec<BoundTypeId>,
    pub(super) trailing: Vec<TrailingSelection>,
}

pub(super) fn segments(
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
) -> Result<Vec<NameSegment>, TypeBindingError> {
    let mut segments = Vec::<NameSegment>::new();
    for element in tree.children(node) {
        match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => {
                segments.push(NameSegment {
                    token: *token,
                    arguments: Vec::new(),
                });
            }
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == NodeKind::SelfType) =>
            {
                let token = direct_identifier(tree, *child)
                    .ok_or(TypeBindingError::InvalidSyntax(*child))?;
                segments.push(NameSegment {
                    token,
                    arguments: Vec::new(),
                });
            }
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == NodeKind::TypeArguments) =>
            {
                let segment = segments
                    .last_mut()
                    .ok_or(TypeBindingError::InvalidSyntax(node))?;
                segment.arguments = direct_nodes(tree, *child, NodeKind::Type)
                    .into_iter()
                    .map(|argument| {
                        values
                            .get(&argument)
                            .copied()
                            .ok_or(TypeBindingError::InvalidSyntax(argument))
                    })
                    .collect::<Result<_, _>>()?;
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
        }
    }
    if segments.is_empty() {
        Err(TypeBindingError::InvalidSyntax(node))
    } else {
        Ok(segments)
    }
}

pub(super) fn resolve_exported(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    node: NodeId,
    segments: Vec<NameSegment>,
) -> Result<ResolvedEntityPath, TypeBindingError> {
    let from = declaration_module(namespaces, declaration)?;
    let mut segments = segments.into_iter();
    let first = segments
        .next()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let name = token_symbol(namespaces, tree, first.token)?;
    let mut entity = namespaces
        .lookup_local(from, name)
        .ok_or(TypeBindingError::UnknownName(node))?;
    projection::reference(namespaces, tree, entity, first.token)?;
    let mut arguments = first.arguments;

    while let ExportedEntity::Module(module) = entity {
        if !arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        let segment = segments
            .next()
            .ok_or(TypeBindingError::InvalidTypeEntity(node))?;
        let name = token_symbol(namespaces, tree, segment.token)?;
        entity = namespaces
            .lookup_export(from, module, name)
            .ok_or(TypeBindingError::UnknownName(node))?;
        projection::reference(namespaces, tree, entity, segment.token)?;
        arguments = segment.arguments;
    }

    let trailing = segments
        .map(|segment| {
            Ok(TrailingSelection {
                name: token_symbol(namespaces, tree, segment.token)?,
                arguments: segment.arguments,
            })
        })
        .collect::<Result<_, TypeBindingError>>()?;
    Ok(ResolvedEntityPath {
        entity,
        arguments,
        trailing,
    })
}

fn direct_identifier(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => Some(*token),
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
