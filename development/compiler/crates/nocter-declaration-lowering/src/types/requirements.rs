use std::collections::HashMap;

use nocter_declarations::{ExpansionCapability, RequirementSubject};
use nocter_model::{BorrowCapability, GenericParameterId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::context::token_symbol;
use super::normalization_origins::NormalizationOrigins;
use super::{
    BoundCapability, BoundRequirementKind, BoundTypeId, BoundTypeKind, TypeBindingError,
    TypeBindingRule, projection, push,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn bind_all(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    root: NodeId,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &HashMap<NodeId, BoundTypeId>,
    capabilities: &HashMap<NodeId, BoundCapability>,
    origins: &mut NormalizationOrigins,
) -> Result<Vec<BoundRequirementKind>, TypeBindingError> {
    let mut result = Vec::new();
    for container in requirement_containers(tree, root) {
        match tree.node(container).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::WhereClause) => {
                for predicate in predicate_nodes(tree, container) {
                    bind_predicate(
                        namespaces,
                        declaration,
                        tree,
                        predicate,
                        kinds,
                        roots,
                        capabilities,
                        origins,
                        &mut result,
                    )?;
                }
            }
            Some(NodeKind::InterfaceBounds) => {
                bind_associated_bounds(
                    namespaces,
                    declaration,
                    tree,
                    container,
                    capabilities,
                    &mut result,
                )?;
            }
            _ => return Err(invalid_requirement(container)),
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn bind_predicate(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    predicate: NodeId,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &HashMap<NodeId, BoundTypeId>,
    capabilities: &HashMap<NodeId, BoundCapability>,
    origins: &mut NormalizationOrigins,
    result: &mut Vec<BoundRequirementKind>,
) -> Result<(), TypeBindingError> {
    match tree.node(predicate).map(nocter_syntax::SyntaxNode::kind) {
        Some(NodeKind::CapabilityPredicate) => {
            let subject = generic_from_token(
                namespaces,
                declaration,
                tree,
                direct_identifier(tree, predicate).ok_or(invalid_requirement(predicate))?,
            )?;
            for capability in direct_nodes(tree, predicate, NodeKind::Capability) {
                result.push(BoundRequirementKind::Capability {
                    subject: RequirementSubject::GenericParameter(subject),
                    capability: capabilities
                        .get(&capability)
                        .cloned()
                        .ok_or(invalid_requirement(predicate))?,
                });
            }
        }
        Some(NodeKind::CopyPredicate) => {
            let token = direct_identifiers(tree, predicate)
                .get(1)
                .copied()
                .ok_or(invalid_requirement(predicate))?;
            result.push(BoundRequirementKind::Copy(generic_from_token(
                namespaces,
                declaration,
                tree,
                token,
            )?));
        }
        Some(NodeKind::TypeEqualityPredicate) => {
            let position = result.len();
            bind_equality(
                namespaces,
                declaration,
                tree,
                predicate,
                kinds,
                roots,
                result,
            )?;
            if matches!(
                result.get(position),
                Some(BoundRequirementKind::TypeEquality { .. })
            ) {
                origins.record_requirement(declaration, position, SyntaxOrigin::Node(predicate));
            }
        }
        Some(NodeKind::OperatorPredicate) => {
            bind_operator(tree, predicate, kinds, roots, result)?;
        }
        Some(NodeKind::CoercionPredicate) => {
            let types = bound_types(tree, predicate, roots)?;
            let [referent, target] = types.as_slice() else {
                return Err(invalid_requirement(predicate));
            };
            let capability = borrow_capability(tree, predicate)?;
            let source = push(
                kinds,
                BoundTypeKind::Borrow {
                    capability,
                    referent: *referent,
                },
            );
            result.push(BoundRequirementKind::Coercion {
                source,
                target: *target,
            });
        }
        Some(NodeKind::ExpansionPredicate) => {
            let token = direct_identifier(tree, predicate).ok_or(invalid_requirement(predicate))?;
            let source = generic_from_token(namespaces, declaration, tree, token)?;
            let types = bound_types(tree, predicate, roots)?;
            let [result_type] = types.as_slice() else {
                return Err(invalid_requirement(predicate));
            };
            result.push(BoundRequirementKind::Expansion {
                capability: expansion_capability(tree, predicate),
                source,
                result: *result_type,
            });
        }
        _ => return Err(invalid_requirement(predicate)),
    }
    Ok(())
}

fn bind_equality(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    predicate: NodeId,
    kinds: &[BoundTypeKind],
    roots: &HashMap<NodeId, BoundTypeId>,
    result: &mut Vec<BoundRequirementKind>,
) -> Result<(), TypeBindingError> {
    let types = bound_types(tree, predicate, roots)?;
    let [left, right] = types.as_slice() else {
        return Err(invalid_requirement(predicate));
    };
    let (left, right) = (*left, *right);
    let is_pattern_owner = namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .is_some_and(|surface| {
            matches!(
                surface.kind(),
                SurfaceDeclarationKind::Instance | SurfaceDeclarationKind::Conformance
            )
        });
    let own = namespaces
        .imports
        .generics
        .own(declaration)
        .unwrap_or_default();
    if is_pattern_owner
        && let Some(BoundTypeKind::GenericParameter(parameter)) = kinds.get(left.index())
        && own.contains(parameter)
    {
        if contains_generic(kinds, right, *parameter) {
            return Err(TypeBindingError::rule(
                TypeBindingRule::RecursiveBinderRefinement,
                SyntaxOrigin::Node(predicate),
            ));
        }
        result.push(BoundRequirementKind::BinderRefinement {
            parameter: *parameter,
            replacement: right,
        });
    } else {
        result.push(BoundRequirementKind::TypeEquality { left, right });
    }
    Ok(())
}

fn bind_operator(
    tree: &SyntaxTree,
    predicate: NodeId,
    kinds: &[BoundTypeKind],
    roots: &HashMap<NodeId, BoundTypeId>,
    result: &mut Vec<BoundRequirementKind>,
) -> Result<(), TypeBindingError> {
    let types = bound_types(tree, predicate, roots)?;
    let [first, second, third] = types.as_slice() else {
        return Err(invalid_requirement(predicate));
    };
    if has_punctuation(tree, predicate, Punctuation::EqualEqual)
        || has_punctuation(tree, predicate, Punctuation::Less)
    {
        let left = generic_type(kinds, *first, predicate)?;
        let right = generic_type(kinds, *second, predicate)?;
        if left != right
            || !matches!(
                kinds.get(third.index()),
                Some(BoundTypeKind::Builtin(nocter_model::BuiltinType::Bool))
            )
        {
            return Err(invalid_requirement(predicate));
        }
        result.push(
            if has_punctuation(tree, predicate, Punctuation::EqualEqual) {
                BoundRequirementKind::Equality { operand: left }
            } else {
                BoundRequirementKind::Ordering { operand: left }
            },
        );
    } else if has_punctuation(tree, predicate, Punctuation::LeftBracket) {
        result.push(BoundRequirementKind::Index {
            capability: borrow_capability(tree, predicate)?,
            container: generic_type(kinds, *first, predicate)?,
            index: *second,
            result: *third,
        });
    } else {
        return Err(invalid_requirement(predicate));
    }
    Ok(())
}

fn bind_associated_bounds(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    bounds: NodeId,
    capabilities: &HashMap<NodeId, BoundCapability>,
    result: &mut Vec<BoundRequirementKind>,
) -> Result<(), TypeBindingError> {
    let Some(ReservedEntity::AssociatedType(subject)) = namespaces
        .imports
        .generics
        .headers
        .reserved
        .entity(declaration)
    else {
        return Err(invalid_requirement(bounds));
    };
    for capability in direct_nodes(tree, bounds, NodeKind::Capability) {
        result.push(BoundRequirementKind::Capability {
            subject: RequirementSubject::AssociatedType(subject),
            capability: capabilities
                .get(&capability)
                .cloned()
                .ok_or(invalid_requirement(bounds))?,
        });
    }
    Ok(())
}

fn generic_from_token(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<GenericParameterId, TypeBindingError> {
    let name = token_symbol(namespaces, tree, token)?;
    let parameter = namespaces
        .imports
        .generics
        .lookup(declaration, name)
        .ok_or(TypeBindingError::rule(
            TypeBindingRule::InvalidRequirement,
            SyntaxOrigin::Token(token),
        ))?;
    projection::generic(namespaces, tree, parameter, token)?;
    Ok(parameter)
}

fn generic_type(
    kinds: &[BoundTypeKind],
    ty: BoundTypeId,
    predicate: NodeId,
) -> Result<GenericParameterId, TypeBindingError> {
    match kinds.get(ty.index()) {
        Some(BoundTypeKind::GenericParameter(parameter)) => Ok(*parameter),
        _ => Err(invalid_requirement(predicate)),
    }
}

fn contains_generic(
    kinds: &[BoundTypeKind],
    root: BoundTypeId,
    parameter: GenericParameterId,
) -> bool {
    let mut pending = vec![root];
    while let Some(current) = pending.pop() {
        match &kinds[current.index()] {
            BoundTypeKind::GenericParameter(candidate) if *candidate == parameter => return true,
            BoundTypeKind::Nominal { arguments, .. }
            | BoundTypeKind::Opaque { arguments, .. }
            | BoundTypeKind::Alias { arguments, .. } => {
                pending.extend(arguments.iter().copied());
            }
            BoundTypeKind::AssociatedSelection { base, .. }
            | BoundTypeKind::Pointer(base)
            | BoundTypeKind::Borrow { referent: base, .. }
            | BoundTypeKind::Slice(base)
            | BoundTypeKind::FixedArray { element: base, .. }
            | BoundTypeKind::Optional(base)
            | BoundTypeKind::Fallible(base) => pending.push(*base),
            BoundTypeKind::Callable(callable) => {
                pending.push(callable.result());
                pending.extend(callable.parameters().iter().copied());
            }
            BoundTypeKind::Builtin(_)
            | BoundTypeKind::GenericParameter(_)
            | BoundTypeKind::SelfType(_) => {}
        }
    }
    false
}

fn requirement_containers(tree: &SyntaxTree, root: NodeId) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut pending: Vec<_> = tree.children(root).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let Some(kind) = tree.node(node).map(nocter_syntax::SyntaxNode::kind) else {
            continue;
        };
        if matches!(kind, NodeKind::WhereClause | NodeKind::InterfaceBounds) {
            found.push(node);
        } else if kind != NodeKind::Block && !is_declaration(kind) {
            pending.extend(tree.children(node).iter().rev().copied());
        }
    }
    found
}

fn predicate_nodes(tree: &SyntaxTree, clause: NodeId) -> Vec<NodeId> {
    tree.children(clause)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            _ => None,
        })
        .collect()
}

fn bound_types(
    tree: &SyntaxTree,
    node: NodeId,
    roots: &HashMap<NodeId, BoundTypeId>,
) -> Result<Vec<BoundTypeId>, TypeBindingError> {
    direct_nodes(tree, node, NodeKind::Type)
        .into_iter()
        .map(|ty| roots.get(&ty).copied().ok_or(invalid_requirement(node)))
        .collect()
}

fn borrow_capability(
    tree: &SyntaxTree,
    predicate: NodeId,
) -> Result<BorrowCapability, TypeBindingError> {
    if has_punctuation(tree, predicate, Punctuation::ReadWrite) {
        Ok(BorrowCapability::ReadWrite)
    } else if has_punctuation(tree, predicate, Punctuation::Ampersand) {
        Ok(BorrowCapability::Readonly)
    } else {
        Err(invalid_requirement(predicate))
    }
}

fn expansion_capability(tree: &SyntaxTree, predicate: NodeId) -> ExpansionCapability {
    if has_punctuation(tree, predicate, Punctuation::ReadWrite) {
        ExpansionCapability::ReadWrite
    } else if has_punctuation(tree, predicate, Punctuation::Ampersand) {
        ExpansionCapability::Readonly
    } else {
        ExpansionCapability::Owned
    }
}

fn invalid_requirement(node: NodeId) -> TypeBindingError {
    TypeBindingError::rule(
        TypeBindingRule::InvalidRequirement,
        SyntaxOrigin::Node(node),
    )
}

fn direct_identifier(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    direct_identifiers(tree, node).into_iter().next()
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

const fn is_declaration(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::PrimitiveDeclaration
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
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::ConformDeclaration
            | NodeKind::AssociatedTypeBinding
            | NodeKind::ConformMethod
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
    )
}
