use std::collections::{HashMap, HashSet};

use nocter_declarations::ExportedEntity;
use nocter_model::{AssociatedTypeId, GenericParameterId, OpaqueTypeId};
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::context::{declaration_module, require_arity, token_symbol};
use super::{BoundOpaqueResult, BoundTypeId, BoundTypeKind, TypeBindingError, projection, push};

#[derive(Clone, Copy)]
pub(super) struct OpaqueSyntax {
    pub(super) declaration: SurfaceDeclarationId,
    pub(super) node: NodeId,
    pub(super) callable_tail: NodeId,
    pub(super) definition: OpaqueTypeId,
}

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    syntax: OpaqueSyntax,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &HashMap<NodeId, BoundTypeId>,
) -> Result<BoundOpaqueResult, TypeBindingError> {
    let OpaqueSyntax {
        declaration,
        node,
        callable_tail,
        definition,
    } = syntax;
    let interface_token = direct_identifiers(tree, node)
        .get(1)
        .copied()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let name = token_symbol(namespaces, tree, interface_token)?;
    let module = declaration_module(namespaces, declaration)?;
    let ExportedEntity::Interface(interface) = namespaces
        .lookup_local(module, name)
        .ok_or(TypeBindingError::UnknownName(node))?
    else {
        return Err(TypeBindingError::InvalidTypeEntity(node));
    };
    projection::reference(
        namespaces,
        tree,
        ExportedEntity::Interface(interface),
        interface_token,
    )?;

    let mut arguments = Vec::new();
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    if let Some(container) = direct_node(tree, node, NodeKind::OpaqueArguments) {
        for argument in direct_nodes(tree, container, NodeKind::OpaqueArgument) {
            let ty_node = direct_node(tree, argument, NodeKind::Type)
                .ok_or(TypeBindingError::InvalidSyntax(argument))?;
            let ty = roots
                .get(&ty_node)
                .copied()
                .ok_or(TypeBindingError::InvalidSyntax(ty_node))?;
            if has_punctuation(tree, argument, Punctuation::Equal) {
                let token = direct_identifiers(tree, argument)
                    .into_iter()
                    .next()
                    .ok_or(TypeBindingError::InvalidSyntax(argument))?;
                let associated = associated_type(namespaces, declaration, interface, tree, token)?;
                if !seen.insert(associated) {
                    return Err(TypeBindingError::DuplicateOpaqueBinding(argument));
                }
                bindings.push((associated, ty));
            } else {
                if !bindings.is_empty() {
                    return Err(TypeBindingError::InvalidTypeArguments(argument));
                }
                arguments.push(ty);
            }
        }
    }
    require_arity(
        namespaces,
        node,
        ReservedEntity::Interface(interface),
        arguments.len(),
    )?;

    let generic_parameters = visible_generics(namespaces, declaration);
    let generic_arguments: Box<_> = generic_parameters
        .iter()
        .copied()
        .map(|parameter| push(kinds, BoundTypeKind::GenericParameter(parameter)))
        .collect();
    let mut result = push(
        kinds,
        BoundTypeKind::Opaque {
            definition,
            arguments: generic_arguments,
        },
    );
    for element in tree.children(callable_tail) {
        let SyntaxElement::Token(token) = element else {
            continue;
        };
        match token.kind() {
            TokenKind::Punctuation(Punctuation::Question) => {
                result = push(kinds, BoundTypeKind::Optional(result));
            }
            TokenKind::Punctuation(Punctuation::Bang) => {
                result = push(kinds, BoundTypeKind::Fallible(result));
            }
            _ => {}
        }
    }
    Ok(BoundOpaqueResult {
        generic_parameters,
        interface,
        arguments: arguments.into_boxed_slice(),
        associated_types: bindings.into_boxed_slice(),
        result,
    })
}

fn associated_type(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    interface: nocter_model::InterfaceId,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<AssociatedTypeId, TypeBindingError> {
    let name = token_symbol(namespaces, tree, token)?;
    let reserved = &namespaces.imports.generics.headers.reserved;
    let found = reserved
        .entities
        .iter()
        .copied()
        .enumerate()
        .find_map(|(index, entity)| {
            let ReservedEntity::AssociatedType(associated) = entity? else {
                return None;
            };
            let owner = reserved.declarations[index].owner()?;
            (reserved.entity(owner) == Some(ReservedEntity::Interface(interface))
                && namespaces.imports.generics.headers.names[index] == Some(name))
            .then_some(associated)
        })
        .ok_or(TypeBindingError::UnknownName(declaration_node(
            namespaces,
            declaration,
        )?))?;
    projection::associated(namespaces, tree, found, token)?;
    Ok(found)
}

fn declaration_node(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<NodeId, TypeBindingError> {
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .map(|surface| surface.node())
        .ok_or(TypeBindingError::MissingSource(declaration))
}

fn visible_generics(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Box<[GenericParameterId]> {
    let mut parameters: Vec<_> = namespaces
        .imports
        .generics
        .visible
        .get(declaration.index())
        .into_iter()
        .flatten()
        .map(|(_, parameter)| *parameter)
        .collect();
    parameters.sort_unstable();
    parameters.dedup();
    parameters.into_boxed_slice()
}

fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    direct_nodes(tree, node, kind).into_iter().next()
}

fn direct_nodes(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
        .collect()
}

fn direct_identifiers(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
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
                if token.kind() == TokenKind::Punctuation(punctuation)
        )
    })
}
