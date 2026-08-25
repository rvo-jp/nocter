use std::collections::{BTreeMap, HashMap};

use nocter_declarations::ExportedEntity;
use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin, Symbol};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::binding_arena::BindingArena;
use super::context::{require_arity, token_symbol, token_text};
use super::names::{resolve_exported, segments};
use super::normalization_origins::NormalizationOrigins;
use super::{
    BoundCallableType, BoundTypeId, BoundTypeKind, TypeBindingError, TypeBindingRule, projection,
    push,
};

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    root: NodeId,
    arena: &mut BindingArena,
) -> Result<BoundTypeId, TypeBindingError> {
    let mut values = HashMap::new();
    let mut pending = vec![(root, false)];
    while let Some((node, expanded)) = pending.pop() {
        if !expanded {
            if let Some(existing) = arena.roots.get(&node).copied() {
                values.insert(node, existing);
                continue;
            }
            pending.push((node, true));
            for child in tree.children(node).iter().rev() {
                if let SyntaxElement::Node(child) = child {
                    pending.push((*child, false));
                }
            }
            continue;
        }
        if let Some(kind) = tree.node(node).map(nocter_syntax::SyntaxNode::kind)
            && let Some(id) = bind_node(
                namespaces,
                declaration,
                tree,
                node,
                kind,
                &values,
                &mut arena.kinds,
                &mut arena.origins,
            )?
        {
            arena
                .origins
                .record_bound_if_absent(id, SyntaxOrigin::Node(node));
            values.insert(node, id);
            if kind == NodeKind::Type {
                arena.roots.insert(node, id);
                arena.root_declarations.insert(node, declaration);
            }
        }
    }
    values
        .get(&root)
        .copied()
        .ok_or(TypeBindingError::InvalidSyntax(root))
}

#[allow(clippy::too_many_arguments)]
fn bind_node(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    node: NodeId,
    kind: NodeKind,
    values: &HashMap<NodeId, BoundTypeId>,
    kinds: &mut Vec<BoundTypeKind>,
    origins: &mut NormalizationOrigins,
) -> Result<Option<BoundTypeId>, TypeBindingError> {
    let result = match kind {
        NodeKind::Type => bind_type_wrapper(tree, node, values, kinds)?,
        NodeKind::NamedType => {
            bind_named(namespaces, declaration, tree, node, values, kinds, origins)?
        }
        NodeKind::PointerType => push(
            kinds,
            BoundTypeKind::Pointer(child_value(tree, node, values)?),
        ),
        NodeKind::BorrowType => push(
            kinds,
            BoundTypeKind::Borrow {
                capability: borrow_capability(tree, node)?,
                referent: child_value(tree, node, values)?,
            },
        ),
        NodeKind::SliceType => push(
            kinds,
            BoundTypeKind::Slice(child_value(tree, node, values)?),
        ),
        NodeKind::FixedArrayType => push(
            kinds,
            BoundTypeKind::FixedArray {
                element: child_value(tree, node, values)?,
                length: array_length(tree, node)?,
            },
        ),
        NodeKind::GroupedType => child_value(tree, node, values)?,
        NodeKind::CallableType => bind_callable(namespaces, tree, node, values, kinds)?,
        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn bind_type_wrapper(
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<BoundTypeId, TypeBindingError> {
    let mut value = child_value(tree, node, values)?;
    for element in tree.children(node) {
        let SyntaxElement::Token(token) = element else {
            continue;
        };
        match token.kind() {
            TokenKind::Punctuation(Punctuation::Question) => {
                value = push(kinds, BoundTypeKind::Optional(value));
            }
            TokenKind::Punctuation(Punctuation::Bang) => {
                value = push(kinds, BoundTypeKind::Fallible(value));
            }
            _ => {}
        }
    }
    Ok(value)
}

fn bind_named(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
    kinds: &mut Vec<BoundTypeKind>,
    origins: &mut NormalizationOrigins,
) -> Result<BoundTypeId, TypeBindingError> {
    let segments = segments(tree, node, values)?;
    let first = segments
        .first()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    if token_text(namespaces, tree, first.token)? == "Self" {
        if !first.arguments.is_empty() {
            return Err(invalid_arguments(first));
        }
        let owner = self_owner(namespaces, declaration).ok_or(TypeBindingError::rule(
            TypeBindingRule::InvalidSelfType,
            SyntaxOrigin::Token(first.token),
        ))?;
        let base = push(kinds, BoundTypeKind::SelfType(owner));
        return bind_associated_tail(namespaces, tree, base, &segments[1..], kinds, origins);
    }

    let name = token_symbol(namespaces, tree, first.token)?;
    if let Some(parameter) = namespaces.imports.generics.lookup(declaration, name) {
        if !first.arguments.is_empty() {
            return Err(invalid_arguments(first));
        }
        projection::generic(namespaces, tree, parameter, first.token)?;
        let base = push(kinds, BoundTypeKind::GenericParameter(parameter));
        return bind_associated_tail(namespaces, tree, base, &segments[1..], kinds, origins);
    }

    let path = resolve_exported(namespaces, declaration, tree, node, segments)?;
    let mut current = bind_entity(
        namespaces,
        path.entity_token,
        path.arguments_origin,
        path.entity,
        &path.arguments,
        kinds,
    )?;
    for selection in path.trailing {
        if !selection.arguments.is_empty() {
            return Err(TypeBindingError::rule(
                TypeBindingRule::InvalidTypeArguments,
                selection
                    .arguments_origin
                    .map_or(SyntaxOrigin::Token(selection.token), SyntaxOrigin::Node),
            ));
        }
        current = push(
            kinds,
            BoundTypeKind::AssociatedSelection {
                base: current,
                name: selection.name,
            },
        );
        origins.record_bound(current, SyntaxOrigin::Token(selection.token));
    }
    Ok(current)
}

fn bind_entity(
    namespaces: &PreparedNamespaces<'_>,
    token: SyntaxToken,
    arguments_origin: Option<NodeId>,
    entity: ExportedEntity,
    arguments: &[BoundTypeId],
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<BoundTypeId, TypeBindingError> {
    match entity {
        ExportedEntity::BuiltinType(builtin) => {
            if !arguments.is_empty() {
                return Err(TypeBindingError::rule(
                    TypeBindingRule::InvalidTypeArguments,
                    arguments_origin.map_or(SyntaxOrigin::Token(token), SyntaxOrigin::Node),
                ));
            }
            Ok(push(kinds, BoundTypeKind::Builtin(builtin)))
        }
        ExportedEntity::NominalType(definition) => {
            require_arity(
                namespaces,
                arguments_origin.map_or(SyntaxOrigin::Token(token), SyntaxOrigin::Node),
                ReservedEntity::NominalType(definition),
                arguments.len(),
            )?;
            Ok(push(
                kinds,
                BoundTypeKind::Nominal {
                    definition,
                    arguments: arguments.into(),
                },
            ))
        }
        ExportedEntity::TypeAlias(definition) => {
            require_arity(
                namespaces,
                arguments_origin.map_or(SyntaxOrigin::Token(token), SyntaxOrigin::Node),
                ReservedEntity::TypeAlias(definition),
                arguments.len(),
            )?;
            Ok(push(
                kinds,
                BoundTypeKind::Alias {
                    definition,
                    arguments: arguments.into(),
                },
            ))
        }
        ExportedEntity::Module(_)
        | ExportedEntity::Interface(_)
        | ExportedEntity::Constant(_)
        | ExportedEntity::Callable(_) => Err(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeEntity,
            SyntaxOrigin::Token(token),
        )),
    }
}

fn bind_associated_tail(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    mut base: BoundTypeId,
    segments: &[super::names::NameSegment],
    kinds: &mut Vec<BoundTypeKind>,
    origins: &mut NormalizationOrigins,
) -> Result<BoundTypeId, TypeBindingError> {
    for segment in segments {
        if !segment.arguments.is_empty() {
            return Err(TypeBindingError::rule(
                TypeBindingRule::InvalidTypeArguments,
                segment
                    .arguments_origin
                    .map_or(SyntaxOrigin::Token(segment.token), SyntaxOrigin::Node),
            ));
        }
        base = push(
            kinds,
            BoundTypeKind::AssociatedSelection {
                base,
                name: token_symbol(namespaces, tree, segment.token)?,
            },
        );
        origins.record_bound(base, SyntaxOrigin::Token(segment.token));
    }
    Ok(base)
}

fn bind_callable(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<BoundTypeId, TypeBindingError> {
    let capability = match direct_punctuation(tree, node) {
        Some(Punctuation::Ampersand) => CallableCapability::Readonly,
        Some(Punctuation::ReadWrite) => CallableCapability::ReadWrite,
        _ => CallableCapability::Owned,
    };
    let parameters_node = direct_node(tree, node, NodeKind::CallableParameters)
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let mut parameters = Vec::new();
    let mut named_parameters = Vec::new();
    let mut has_argument_pack = false;
    let mut names = BTreeMap::new();
    let parameter_nodes = direct_nodes(tree, parameters_node, NodeKind::CallableParameter);
    for (position, parameter) in parameter_nodes.iter().copied().enumerate() {
        let ty = descendant_value(tree, parameter, values)
            .ok_or(TypeBindingError::InvalidSyntax(parameter))?;
        let pack = direct_node(tree, parameter, NodeKind::ArgumentPackModifier).is_some();
        if pack && (has_argument_pack || position + 1 != parameter_nodes.len()) {
            return Err(TypeBindingError::InvalidSyntax(parameter));
        }
        has_argument_pack |= pack;
        let position = parameters.len();
        parameters.push(ty);
        let parameter_name = callable_parameter_name(namespaces, tree, parameter)?;
        named_parameters.push(parameter_name.is_some());
        if let Some((name, token)) = parameter_name
            && let Some((_, first)) = names.insert(name, (position, token))
        {
            return Err(TypeBindingError::duplicate_rule(
                TypeBindingRule::DuplicateCallableParameter,
                SyntaxOrigin::Token(first),
                SyntaxOrigin::Token(token),
            ));
        }
    }
    let result = direct_nodes(tree, node, NodeKind::Type)
        .into_iter()
        .find_map(|candidate| values.get(&candidate).copied())
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let explicit_origins = direct_node(tree, node, NodeKind::ProvenanceClause)
        .map(|clause| callable_origins(namespaces, tree, clause, &names))
        .transpose()?;
    Ok(push(
        kinds,
        BoundTypeKind::Callable(BoundCallableType {
            capability,
            parameters: parameters.into_boxed_slice(),
            has_argument_pack,
            result,
            named_parameters: named_parameters.into_boxed_slice(),
            explicit_origins,
        }),
    ))
}

fn callable_origins(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    clause: NodeId,
    names: &BTreeMap<Symbol, (usize, SyntaxToken)>,
) -> Result<Box<[ParameterOrigin]>, TypeBindingError> {
    let mut tokens = identifier_tokens(tree, clause).into_iter();
    tokens
        .next()
        .ok_or(TypeBindingError::InvalidSyntax(clause))?;
    let mut origins = BTreeMap::new();
    for token in tokens {
        let name = token_symbol(namespaces, tree, token)?;
        let position =
            names
                .get(&name)
                .map(|(position, _)| *position)
                .ok_or(TypeBindingError::rule(
                    TypeBindingRule::UnknownProvenanceOrigin,
                    SyntaxOrigin::Token(token),
                ))?;
        if let Some(first) = origins.insert(position, token) {
            return Err(TypeBindingError::duplicate_rule(
                TypeBindingRule::DuplicateProvenanceOrigin,
                SyntaxOrigin::Token(first),
                SyntaxOrigin::Token(token),
            ));
        }
    }
    Ok(origins
        .into_keys()
        .map(ParameterOrigin::new)
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn callable_parameter_name(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    parameter: NodeId,
) -> Result<Option<(Symbol, SyntaxToken)>, TypeBindingError> {
    let has_colon = tree.children(parameter).iter().any(|element| {
        matches!(
            element,
            SyntaxElement::Token(token)
                if token.kind() == TokenKind::Punctuation(Punctuation::Colon)
        )
    });
    if !has_colon {
        return Ok(None);
    }
    direct_identifier(tree, parameter)
        .map(|token| token_symbol(namespaces, tree, token).map(|name| (name, token)))
        .transpose()
}

fn self_owner(
    namespaces: &PreparedNamespaces<'_>,
    mut declaration: SurfaceDeclarationId,
) -> Option<ReservedEntity> {
    let reserved = &namespaces.imports.generics.headers.reserved;
    loop {
        let surface = *reserved.declarations.get(declaration.index())?;
        if matches!(
            surface.kind(),
            SurfaceDeclarationKind::Struct
                | SurfaceDeclarationKind::Enum
                | SurfaceDeclarationKind::Interface
                | SurfaceDeclarationKind::Construction
                | SurfaceDeclarationKind::Instance
                | SurfaceDeclarationKind::Conformance
                | SurfaceDeclarationKind::Drop
        ) {
            return reserved.entity(declaration);
        }
        declaration = surface.owner()?;
    }
}

fn child_value(
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
) -> Result<BoundTypeId, TypeBindingError> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child) => values.get(child).copied(),
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidSyntax(node))
}

fn descendant_value(
    tree: &SyntaxTree,
    node: NodeId,
    values: &HashMap<NodeId, BoundTypeId>,
) -> Option<BoundTypeId> {
    let mut pending: Vec<_> = tree.children(node).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        if let SyntaxElement::Node(child) = element {
            if let Some(value) = values.get(&child) {
                return Some(*value);
            }
            pending.extend(tree.children(child).iter().rev().copied());
        }
    }
    None
}

fn borrow_capability(
    tree: &SyntaxTree,
    node: NodeId,
) -> Result<BorrowCapability, TypeBindingError> {
    match direct_punctuation(tree, node) {
        Some(Punctuation::Ampersand) => Ok(BorrowCapability::Readonly),
        Some(Punctuation::ReadWrite) => Ok(BorrowCapability::ReadWrite),
        _ => Err(TypeBindingError::InvalidSyntax(node)),
    }
}

fn array_length(tree: &SyntaxTree, node: NodeId) -> Result<NodeId, TypeBindingError> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|node| node.kind() == NodeKind::Expression) =>
            {
                Some(*child)
            }
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidSyntax(node))
}

fn invalid_arguments(segment: &super::names::NameSegment) -> TypeBindingError {
    TypeBindingError::rule(
        TypeBindingRule::InvalidTypeArguments,
        segment
            .arguments_origin
            .map_or(SyntaxOrigin::Token(segment.token), SyntaxOrigin::Node),
    )
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

fn direct_punctuation(tree: &SyntaxTree, node: NodeId) -> Option<Punctuation> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => match token.kind() {
                TokenKind::Punctuation(punctuation) => Some(punctuation),
                _ => None,
            },
            _ => None,
        })
}
