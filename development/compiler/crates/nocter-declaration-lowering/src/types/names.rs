use std::collections::HashMap;

use nocter_declarations::ExportedEntity;
use nocter_model::Symbol;
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind, direct_identifier,
    direct_nodes,
};

use crate::{PreparedNamespaces, SurfaceDeclarationId};

use super::context::{declaration_module, declaration_source, token_symbol};
use super::{BoundTypeId, TypeBindingError, TypeBindingRule, projection};

pub(super) struct NameSegment {
    pub(super) token: SyntaxToken,
    pub(super) arguments: Vec<BoundTypeId>,
    pub(super) arguments_origin: Option<NodeId>,
}

pub(super) struct TrailingSelection {
    pub(super) name: Symbol,
    pub(super) arguments: Vec<BoundTypeId>,
    pub(super) token: SyntaxToken,
    pub(super) arguments_origin: Option<NodeId>,
}

pub(super) struct ResolvedEntityPath {
    pub(super) entity: ExportedEntity,
    pub(super) entity_token: SyntaxToken,
    pub(super) arguments: Vec<BoundTypeId>,
    pub(super) arguments_origin: Option<NodeId>,
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
            SyntaxElement::Token(token) if is_type_name(token.kind()) => {
                segments.push(NameSegment {
                    token: *token,
                    arguments: Vec::new(),
                    arguments_origin: None,
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
                    arguments_origin: None,
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
                segment.arguments_origin = Some(*child);
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

const fn is_type_name(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::Keyword(nocter_syntax::Keyword::Void | nocter_syntax::Keyword::Never)
    )
}

pub(super) fn resolve_exported(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    node: NodeId,
    segments: Vec<NameSegment>,
) -> Result<ResolvedEntityPath, TypeBindingError> {
    let from = declaration_module(namespaces, declaration)?;
    let source = declaration_source(namespaces, declaration)?;
    let mut segments = segments.into_iter();
    let first = segments
        .next()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let name = token_symbol(namespaces, tree, first.token)?;
    let mut entity = namespaces
        .lookup_local(source, name)
        .ok_or(TypeBindingError::rule(
            TypeBindingRule::UnknownTypeContextName,
            SyntaxOrigin::Token(first.token),
        ))?;
    projection::reference(namespaces, tree, entity, first.token)?;
    let mut arguments = first.arguments;
    let mut arguments_origin = first.arguments_origin;
    let mut entity_token = first.token;

    while let ExportedEntity::Module(module) = entity {
        if !arguments.is_empty() {
            return Err(TypeBindingError::rule(
                TypeBindingRule::InvalidTypeArguments,
                SyntaxOrigin::Node(arguments_origin.unwrap_or(node)),
            ));
        }
        let segment = segments.next().ok_or(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeEntity,
            SyntaxOrigin::Token(entity_token),
        ))?;
        let name = token_symbol(namespaces, tree, segment.token)?;
        entity = namespaces
            .lookup_export(from, module, name)
            .ok_or(TypeBindingError::rule(
                TypeBindingRule::UnknownTypeContextName,
                SyntaxOrigin::Token(segment.token),
            ))?;
        projection::reference(namespaces, tree, entity, segment.token)?;
        arguments = segment.arguments;
        arguments_origin = segment.arguments_origin;
        entity_token = segment.token;
    }

    let trailing = segments
        .map(|segment| {
            Ok(TrailingSelection {
                name: token_symbol(namespaces, tree, segment.token)?,
                arguments: segment.arguments,
                token: segment.token,
                arguments_origin: segment.arguments_origin,
            })
        })
        .collect::<Result<_, TypeBindingError>>()?;
    Ok(ResolvedEntityPath {
        entity,
        entity_token,
        arguments,
        arguments_origin,
        trailing,
    })
}
