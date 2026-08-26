use std::collections::HashMap;

use nocter_model::{AssociatedTypeId, GenericParameterId, OpaqueTypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, SurfaceDeclarationId};

use super::context::token_symbol;
use super::{
    BoundInterfaceApplication, BoundOpaqueResult, BoundTypeId, BoundTypeKind, TypeBindingError,
    TypeBindingRule, binding_arena::BindingArena, projection, push,
};

#[derive(Clone, Copy)]
pub(super) struct OpaqueSyntax {
    pub(super) declaration: SurfaceDeclarationId,
    pub(super) node: NodeId,
    pub(super) callable_tail: NodeId,
    pub(super) definition: OpaqueTypeId,
}

struct BoundOpaqueArguments {
    positional: Vec<BoundTypeId>,
    associated: Vec<(AssociatedTypeId, BoundTypeId)>,
}

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    syntax: OpaqueSyntax,
    interface_applications: &HashMap<NodeId, BoundInterfaceApplication>,
    arena: &mut BindingArena,
) -> Result<BoundOpaqueResult, TypeBindingError> {
    let OpaqueSyntax {
        declaration,
        node,
        callable_tail,
        definition,
    } = syntax;
    let application = direct_node(tree, node, NodeKind::InterfaceApplication)
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let BoundInterfaceApplication {
        definition: interface,
        arguments: positional,
    } = interface_applications
        .get(&application)
        .ok_or(TypeBindingError::InvalidSyntax(application))?;
    let arguments = bind_opaque_arguments(
        namespaces,
        tree,
        application,
        *interface,
        positional.to_vec(),
        &arena.roots,
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
        interface: *interface,
        arguments: arguments.positional.into_boxed_slice(),
        associated_types: arguments.associated.into_boxed_slice(),
        result,
    })
}

fn bind_opaque_arguments(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    application: NodeId,
    interface: nocter_model::InterfaceId,
    positional: Vec<BoundTypeId>,
    roots: &HashMap<NodeId, BoundTypeId>,
) -> Result<BoundOpaqueArguments, TypeBindingError> {
    let mut associated = Vec::new();
    let mut seen = HashMap::new();
    if let Some(bindings) = direct_node(tree, application, NodeKind::AssociatedBindings) {
        for binding_node in direct_nodes(tree, bindings, NodeKind::AssociatedTypeBinding) {
            let token = direct_identifiers(tree, binding_node)
                .into_iter()
                .next()
                .ok_or(TypeBindingError::InvalidSyntax(binding_node))?;
            let binding = associated_type(namespaces, interface, tree, token)?;
            if let Some(first) = seen.insert(binding, token) {
                return Err(TypeBindingError::duplicate_rule(
                    TypeBindingRule::DuplicateOpaqueBinding,
                    SyntaxOrigin::Token(first),
                    SyntaxOrigin::Token(token),
                ));
            }
            let ty_node = direct_node(tree, binding_node, NodeKind::Type)
                .ok_or(TypeBindingError::InvalidSyntax(binding_node))?;
            let ty = roots
                .get(&ty_node)
                .copied()
                .ok_or(TypeBindingError::InvalidSyntax(ty_node))?;
            associated.push((binding, ty));
        }
    }
    Ok(BoundOpaqueArguments {
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
    let found = namespaces
        .imports
        .generics
        .headers
        .associated_type(interface, name)
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
