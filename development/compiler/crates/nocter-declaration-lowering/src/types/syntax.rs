use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::ExportedEntity;
use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin, Symbol};
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::context::{builtin_type, require_arity, token_symbol, token_text};
use super::names::{resolve_exported, segments};
use super::{BoundCallableType, BoundTypeId, BoundTypeKind, TypeBindingError, projection, push};

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    root: NodeId,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &mut HashMap<NodeId, BoundTypeId>,
    root_declarations: &mut HashMap<NodeId, SurfaceDeclarationId>,
) -> Result<BoundTypeId, TypeBindingError> {
    let mut values = HashMap::new();
    let mut pending = vec![(root, false)];
    while let Some((node, expanded)) = pending.pop() {
        if !expanded {
            if let Some(existing) = roots.get(&node).copied() {
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
            && let Some(id) = bind_node(namespaces, declaration, tree, node, kind, &values, kinds)?
        {
            values.insert(node, id);
            if kind == NodeKind::Type {
                roots.insert(node, id);
                root_declarations.insert(node, declaration);
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
) -> Result<Option<BoundTypeId>, TypeBindingError> {
    let result = match kind {
        NodeKind::Type => bind_type_wrapper(tree, node, values, kinds)?,
        NodeKind::BuiltinType => push(
            kinds,
            BoundTypeKind::Builtin(builtin_type(namespaces, tree, node)?),
        ),
        NodeKind::NamedType => bind_named(namespaces, declaration, tree, node, values, kinds)?,
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
                length: array_length(namespaces, tree, node)?,
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
) -> Result<BoundTypeId, TypeBindingError> {
    let segments = segments(tree, node, values)?;
    let first = segments
        .first()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    if token_text(namespaces, tree, first.token)? == "Self" {
        if !first.arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        let owner =
            self_owner(namespaces, declaration).ok_or(TypeBindingError::InvalidSelfType(node))?;
        let base = push(kinds, BoundTypeKind::SelfType(owner));
        return bind_associated_tail(namespaces, tree, node, base, &segments[1..], kinds);
    }

    let name = token_symbol(namespaces, tree, first.token)?;
    if let Some(parameter) = namespaces.imports.generics.lookup(declaration, name) {
        if !first.arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        projection::generic(namespaces, tree, parameter, first.token)?;
        let base = push(kinds, BoundTypeKind::GenericParameter(parameter));
        return bind_associated_tail(namespaces, tree, node, base, &segments[1..], kinds);
    }

    let path = resolve_exported(namespaces, declaration, tree, node, segments)?;
    let mut current = bind_entity(namespaces, node, path.entity, &path.arguments, kinds)?;
    for selection in path.trailing {
        if !selection.arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        current = push(
            kinds,
            BoundTypeKind::AssociatedSelection {
                base: current,
                name: selection.name,
            },
        );
    }
    Ok(current)
}

fn bind_entity(
    namespaces: &PreparedNamespaces<'_>,
    node: NodeId,
    entity: ExportedEntity,
    arguments: &[BoundTypeId],
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<BoundTypeId, TypeBindingError> {
    match entity {
        ExportedEntity::Module(_) => Err(TypeBindingError::InvalidTypeEntity(node)),
        ExportedEntity::NominalType(definition) => {
            require_arity(
                namespaces,
                node,
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
                node,
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
        ExportedEntity::Interface(_) | ExportedEntity::Callable(_) => {
            Err(TypeBindingError::InvalidTypeEntity(node))
        }
    }
}

fn bind_associated_tail(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    node: NodeId,
    mut base: BoundTypeId,
    segments: &[super::names::NameSegment],
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<BoundTypeId, TypeBindingError> {
    for segment in segments {
        if !segment.arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        base = push(
            kinds,
            BoundTypeKind::AssociatedSelection {
                base,
                name: token_symbol(namespaces, tree, segment.token)?,
            },
        );
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
    let mut names = BTreeMap::new();
    for parameter in direct_nodes(tree, parameters_node, NodeKind::CallableParameter) {
        let ty = descendant_value(tree, parameter, values)
            .ok_or(TypeBindingError::InvalidSyntax(parameter))?;
        let position = parameters.len();
        parameters.push(ty);
        let name = callable_parameter_name(namespaces, tree, parameter)?;
        named_parameters.push(name.is_some());
        if let Some(name) = name
            && names.insert(name, position).is_some()
        {
            return Err(TypeBindingError::DuplicateCallableParameter(node));
        }
    }
    let result = direct_nodes(tree, node, NodeKind::Type)
        .into_iter()
        .find_map(|candidate| values.get(&candidate).copied())
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let explicit_origins = direct_node(tree, node, NodeKind::ProvenanceClause)
        .map(|clause| callable_origins(namespaces, tree, node, clause, &names))
        .transpose()?;
    Ok(push(
        kinds,
        BoundTypeKind::Callable(BoundCallableType {
            capability,
            parameters: parameters.into_boxed_slice(),
            result,
            named_parameters: named_parameters.into_boxed_slice(),
            explicit_origins,
        }),
    ))
}

fn callable_origins(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    callable: NodeId,
    clause: NodeId,
    names: &BTreeMap<Symbol, usize>,
) -> Result<Box<[ParameterOrigin]>, TypeBindingError> {
    let mut tokens = identifier_tokens(tree, clause).into_iter();
    tokens
        .next()
        .ok_or(TypeBindingError::InvalidSyntax(clause))?;
    let mut origins = BTreeSet::new();
    for token in tokens {
        let name = token_symbol(namespaces, tree, token)?;
        let position = names
            .get(&name)
            .copied()
            .ok_or(TypeBindingError::UnknownProvenanceOrigin(callable))?;
        if !origins.insert(position) {
            return Err(TypeBindingError::DuplicateProvenanceOrigin(callable));
        }
    }
    Ok(origins
        .into_iter()
        .map(ParameterOrigin::new)
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn callable_parameter_name(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    parameter: NodeId,
) -> Result<Option<Symbol>, TypeBindingError> {
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
        .map(|token| token_symbol(namespaces, tree, token))
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

fn array_length(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    node: NodeId,
) -> Result<u64, TypeBindingError> {
    let token = tree
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::IntegerLiteral => {
                Some(*token)
            }
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidArrayLength(node))?;
    let text = token_text(namespaces, tree, token)?.replace('_', "");
    let parsed = if let Some(digits) = text.strip_prefix("0x") {
        u64::from_str_radix(digits, 16)
    } else if let Some(digits) = text.strip_prefix("0b") {
        u64::from_str_radix(digits, 2)
    } else {
        text.parse()
    };
    parsed.map_err(|_| TypeBindingError::InvalidArrayLength(node))
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
