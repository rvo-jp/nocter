use std::collections::HashMap;

use nocter_model::{BuiltinType, OpaqueTypeId};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::{
    BoundInterfaceApplication, BoundOpaqueResult, BoundTypeId, BoundTypeKind, TypeBindingError,
    binding_arena::BindingArena, opaque, push, syntax,
};

type BoundResults = (
    HashMap<OpaqueTypeId, BoundOpaqueResult>,
    Box<[Option<BoundTypeId>]>,
);

pub(super) fn bind_all(
    namespaces: &mut PreparedNamespaces<'_>,
    interface_applications: &HashMap<NodeId, BoundInterfaceApplication>,
    arena: &mut BindingArena,
) -> Result<BoundResults, TypeBindingError> {
    let count = namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .len();
    let mut opaque_results = HashMap::new();
    let mut callable_results = vec![None; count];
    for index in 0..count {
        let declaration = SurfaceDeclarationId::from_index(index);
        let surface = namespaces.imports.generics.headers.reserved.declarations[index];
        let Some(entity) = namespaces
            .imports
            .generics
            .headers
            .reserved
            .entity(declaration)
        else {
            continue;
        };
        let tree =
            namespaces.imports.generics.headers.reserved.sources[surface.source().index()].syntax();
        match entity {
            ReservedEntity::OpaqueType(opaque_id) => {
                let owner = surface
                    .owner()
                    .ok_or(TypeBindingError::InvalidSyntax(surface.node()))?;
                let owner_node =
                    namespaces.imports.generics.headers.reserved.declarations[owner.index()].node();
                let tail = find_descendant(tree, owner_node, NodeKind::CallableTail)
                    .ok_or(TypeBindingError::InvalidSyntax(owner_node))?;
                let bound = opaque::bind(
                    namespaces,
                    tree,
                    opaque::OpaqueSyntax {
                        declaration,
                        node: surface.node(),
                        callable_tail: tail,
                        definition: opaque_id,
                    },
                    interface_applications,
                    arena,
                )?;
                callable_results[owner.index()] = Some(bound.result);
                opaque_results.insert(opaque_id, bound);
            }
            ReservedEntity::Callable(_) => {
                if callable_results[index].is_none()
                    && let Some(result) = bind_callable_result(
                        namespaces,
                        declaration,
                        tree,
                        surface.node(),
                        surface.kind(),
                        arena,
                    )?
                {
                    callable_results[index] = Some(result);
                }
            }
            _ => {}
        }
    }
    Ok((opaque_results, callable_results.into_boxed_slice()))
}

#[allow(clippy::too_many_arguments)]
fn bind_callable_result(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    node: NodeId,
    kind: SurfaceDeclarationKind,
    arena: &mut BindingArena,
) -> Result<Option<BoundTypeId>, TypeBindingError> {
    let result_node = match kind {
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::PrimitiveFunction
        | SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::ConstructionFunction
        | SurfaceDeclarationKind::Literal
        | SurfaceDeclarationKind::InherentMethod => {
            let tail = find_descendant(tree, node, NodeKind::CallableTail)
                .ok_or(TypeBindingError::InvalidSyntax(node))?;
            direct_node(tree, tail, NodeKind::Type)
        }
        SurfaceDeclarationKind::Coercion | SurfaceDeclarationKind::Expansion => {
            direct_node(tree, node, NodeKind::Type)
        }
        SurfaceDeclarationKind::Index => direct_node(tree, node, NodeKind::BorrowType),
        SurfaceDeclarationKind::Equality | SurfaceDeclarationKind::Ordering => {
            return Ok(Some(push(
                &mut arena.kinds,
                BoundTypeKind::Builtin(BuiltinType::Bool),
            )));
        }
        _ => return Ok(None),
    };
    let Some(result_node) = result_node else {
        // An opaque result is filled when its separately reserved child is processed.
        return Ok(None);
    };
    if let Some(bound) = arena.roots.get(&result_node).copied() {
        return Ok(Some(bound));
    }
    syntax::bind(namespaces, declaration, tree, result_node, arena).map(Some)
}

fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
}

fn find_descendant(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Option<NodeId> {
    let mut pending: Vec<_> = tree.children(root).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        if tree.node(node).is_some_and(|syntax| syntax.kind() == kind) {
            return Some(node);
        }
        pending.extend(tree.children(node).iter().rev().copied());
    }
    None
}
