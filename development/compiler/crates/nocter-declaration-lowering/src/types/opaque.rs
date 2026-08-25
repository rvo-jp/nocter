use std::collections::HashMap;

use nocter_declarations::ExportedEntity;
use nocter_model::{AssociatedTypeId, GenericParameterId, OpaqueTypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::context::{declaration_source, require_arity, token_symbol};
use super::{
    BoundOpaqueResult, BoundTypeId, BoundTypeKind, TypeBindingError, TypeBindingRule,
    binding_arena::BindingArena, projection, push,
};

#[derive(Clone, Copy)]
pub(super) struct OpaqueSyntax {
    pub(super) declaration: SurfaceDeclarationId,
    pub(super) node: NodeId,
    pub(super) callable_tail: NodeId,
    pub(super) definition: OpaqueTypeId,
}

struct BoundOpaqueArguments {
    node: Option<NodeId>,
    positional: Vec<BoundTypeId>,
    associated: Vec<(AssociatedTypeId, BoundTypeId)>,
}

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    syntax: OpaqueSyntax,
    arena: &mut BindingArena,
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
    let source = declaration_source(namespaces, declaration)?;
    let ExportedEntity::Interface(interface) =
        namespaces
            .lookup_local(source, name)
            .ok_or(TypeBindingError::rule(
                TypeBindingRule::UnknownTypeContextName,
                SyntaxOrigin::Token(interface_token),
            ))?
    else {
        return Err(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeEntity,
            SyntaxOrigin::Token(interface_token),
        ));
    };
    projection::reference(
        namespaces,
        tree,
        ExportedEntity::Interface(interface),
        interface_token,
    )?;

    let arguments = bind_opaque_arguments(namespaces, tree, node, interface, &arena.roots)?;
    require_arity(
        namespaces,
        arguments
            .node
            .map_or(SyntaxOrigin::Token(interface_token), SyntaxOrigin::Node),
        ReservedEntity::Interface(interface),
        arguments.positional.len(),
    )?;

    let generic_parameters = visible_generics(namespaces, declaration);
    let generic_arguments: Box<_> = generic_parameters
        .iter()
        .copied()
        .map(|parameter| push(&mut arena.kinds, BoundTypeKind::GenericParameter(parameter)))
        .collect();
    let mut result = push(
        &mut arena.kinds,
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
                result = push(&mut arena.kinds, BoundTypeKind::Optional(result));
            }
            TokenKind::Punctuation(Punctuation::Bang) => {
                result = push(&mut arena.kinds, BoundTypeKind::Fallible(result));
            }
            _ => {}
        }
    }
    Ok(BoundOpaqueResult {
        generic_parameters,
        interface,
        arguments: arguments.positional.into_boxed_slice(),
        associated_types: arguments.associated.into_boxed_slice(),
        result,
    })
}

fn bind_opaque_arguments(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    opaque: NodeId,
    interface: nocter_model::InterfaceId,
    roots: &HashMap<NodeId, BoundTypeId>,
) -> Result<BoundOpaqueArguments, TypeBindingError> {
    let node = direct_node(tree, opaque, NodeKind::OpaqueArguments);
    let mut positional = Vec::new();
    let mut associated = Vec::new();
    let mut seen = HashMap::new();
    for argument in node
        .into_iter()
        .flat_map(|container| direct_nodes(tree, container, NodeKind::OpaqueArgument))
    {
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
            let binding = associated_type(namespaces, interface, tree, token)?;
            if let Some(first) = seen.insert(binding, token) {
                return Err(TypeBindingError::duplicate_rule(
                    TypeBindingRule::DuplicateOpaqueBinding,
                    SyntaxOrigin::Token(first),
                    SyntaxOrigin::Token(token),
                ));
            }
            associated.push((binding, ty));
        } else {
            if !associated.is_empty() {
                return Err(TypeBindingError::rule(
                    TypeBindingRule::OpaqueArgumentOrder,
                    SyntaxOrigin::Node(argument),
                ));
            }
            positional.push(ty);
        }
    }
    Ok(BoundOpaqueArguments {
        node,
        positional,
        associated,
    })
}

fn associated_type(
    namespaces: &mut PreparedNamespaces<'_>,
    interface: nocter_model::InterfaceId,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<AssociatedTypeId, TypeBindingError> {
    let name = token_symbol(namespaces, tree, token)?;
    let reserved = &namespaces.imports.generics.headers.reserved;
    let found = reserved
        .entities()
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
        .ok_or(TypeBindingError::rule(
            TypeBindingRule::UnknownOpaqueBinding,
            SyntaxOrigin::Token(token),
        ))?;
    projection::associated(namespaces, tree, found, token)?;
    Ok(found)
}

fn visible_generics(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Box<[GenericParameterId]> {
    let mut parameters: Vec<_> = namespaces
        .imports
        .generics
        .visible_ids(declaration)
        .into_iter()
        .flatten()
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
