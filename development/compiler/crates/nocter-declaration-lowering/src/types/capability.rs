use std::collections::HashMap;

use nocter_declarations::ExportedEntity;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::context::require_arity;
use super::names::{resolve_exported, segments};
use super::{BoundCapability, BoundTypeId, BoundTypeKind, TypeBindingError, syntax};

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    capability: NodeId,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &mut HashMap<NodeId, BoundTypeId>,
) -> Result<BoundCapability, TypeBindingError> {
    let child = tree
        .children(capability)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child) => Some(*child),
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidSyntax(capability))?;
    match tree.node(child).map(nocter_syntax::SyntaxNode::kind) {
        Some(NodeKind::CallableType) => {
            let id = syntax::bind(namespaces, declaration, tree, child, kinds, roots)?;
            if matches!(kinds.get(id.index()), Some(BoundTypeKind::Callable(_))) {
                Ok(BoundCapability::Callable(id))
            } else {
                Err(TypeBindingError::InvalidSyntax(capability))
            }
        }
        Some(NodeKind::NamedType) => {
            let path = resolve_exported(
                namespaces,
                declaration,
                tree,
                child,
                segments(tree, child, roots)?,
            )?;
            let ExportedEntity::Interface(definition) = path.entity else {
                return Err(TypeBindingError::InvalidTypeEntity(capability));
            };
            if !path.trailing.is_empty() {
                return Err(TypeBindingError::InvalidTypeEntity(capability));
            }
            require_arity(
                namespaces,
                capability,
                ReservedEntity::Interface(definition),
                path.arguments.len(),
            )?;
            Ok(BoundCapability::Interface {
                definition,
                arguments: path.arguments.into_boxed_slice(),
            })
        }
        _ => Err(TypeBindingError::InvalidSyntax(capability)),
    }
}
