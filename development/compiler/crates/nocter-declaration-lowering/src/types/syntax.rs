use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::ExportedEntity;
use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin, Symbol};
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::{BoundCallableType, BoundTypeId, BoundTypeKind, TypeBindingError, projection, push};

pub(super) fn bind(
    namespaces: &mut PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &SyntaxTree,
    root: NodeId,
    kinds: &mut Vec<BoundTypeKind>,
    roots: &mut HashMap<NodeId, BoundTypeId>,
) -> Result<BoundTypeId, TypeBindingError> {
    let mut values = HashMap::new();
    let mut pending = vec![(root, false)];
    while let Some((node, expanded)) = pending.pop() {
        if !expanded {
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
            BoundTypeKind::Builtin(builtin(namespaces, tree, node)?),
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
    let segments = named_segments(namespaces, tree, node, values)?;
    let first = segments
        .first()
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    let mut current = if token_text(namespaces, tree, first.token)? == "Self" {
        if !first.arguments.is_empty() {
            return Err(TypeBindingError::InvalidTypeArguments(node));
        }
        let owner =
            self_owner(namespaces, declaration).ok_or(TypeBindingError::InvalidSelfType(node))?;
        NameState::Type(push(kinds, BoundTypeKind::SelfType(owner)))
    } else {
        let name = token_symbol(namespaces, tree, first.token)?;
        if let Some(parameter) = namespaces.imports.generics.lookup(declaration, name) {
            if !first.arguments.is_empty() {
                return Err(TypeBindingError::InvalidTypeArguments(node));
            }
            projection::generic(namespaces, tree, parameter, first.token)?;
            NameState::Type(push(kinds, BoundTypeKind::GenericParameter(parameter)))
        } else {
            let module = declaration_module(namespaces, declaration)?;
            let entity = namespaces
                .lookup_local(module, name)
                .ok_or(TypeBindingError::UnknownName(node))?;
            projection::reference(namespaces, tree, entity, first.token)?;
            bind_entity(namespaces, node, entity, &first.arguments, kinds)?
        }
    };

    for segment in segments.iter().skip(1) {
        let name = token_symbol(namespaces, tree, segment.token)?;
        current = match current {
            NameState::Module(module) => {
                let from = declaration_module(namespaces, declaration)?;
                let entity = namespaces
                    .lookup_export(from, module, name)
                    .ok_or(TypeBindingError::UnknownName(node))?;
                projection::reference(namespaces, tree, entity, segment.token)?;
                bind_entity(namespaces, node, entity, &segment.arguments, kinds)?
            }
            NameState::Type(base) => {
                if !segment.arguments.is_empty() {
                    return Err(TypeBindingError::InvalidTypeArguments(node));
                }
                NameState::Type(push(
                    kinds,
                    BoundTypeKind::AssociatedSelection { base, name },
                ))
            }
        };
    }
    match current {
        NameState::Type(ty) => Ok(ty),
        NameState::Module(_) => Err(TypeBindingError::InvalidTypeEntity(node)),
    }
}

fn bind_entity(
    namespaces: &PreparedNamespaces<'_>,
    node: NodeId,
    entity: ExportedEntity,
    arguments: &[BoundTypeId],
    kinds: &mut Vec<BoundTypeKind>,
) -> Result<NameState, TypeBindingError> {
    match entity {
        ExportedEntity::Module(module) => {
            if arguments.is_empty() {
                Ok(NameState::Module(module))
            } else {
                Err(TypeBindingError::InvalidTypeArguments(node))
            }
        }
        ExportedEntity::NominalType(definition) => {
            require_arity(
                namespaces,
                node,
                ReservedEntity::NominalType(definition),
                arguments.len(),
            )?;
            Ok(NameState::Type(push(
                kinds,
                BoundTypeKind::Nominal {
                    definition,
                    arguments: arguments.into(),
                },
            )))
        }
        ExportedEntity::TypeAlias(definition) => {
            require_arity(
                namespaces,
                node,
                ReservedEntity::TypeAlias(definition),
                arguments.len(),
            )?;
            Ok(NameState::Type(push(
                kinds,
                BoundTypeKind::Alias {
                    definition,
                    arguments: arguments.into(),
                },
            )))
        }
        ExportedEntity::Interface(_) | ExportedEntity::Callable(_) => {
            Err(TypeBindingError::InvalidTypeEntity(node))
        }
    }
}

fn require_arity(
    namespaces: &PreparedNamespaces<'_>,
    node: NodeId,
    entity: ReservedEntity,
    actual: usize,
) -> Result<(), TypeBindingError> {
    let generics = &namespaces.imports.generics;
    let expected = generics
        .headers
        .reserved
        .entities
        .iter()
        .position(|candidate| *candidate == Some(entity))
        .and_then(|index| generics.own.get(index))
        .map_or(0, |parameters| parameters.len());
    if expected == actual {
        Ok(())
    } else {
        Err(TypeBindingError::InvalidTypeArguments(node))
    }
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
    let mut names = BTreeMap::new();
    for parameter in direct_nodes(tree, parameters_node, NodeKind::CallableParameter) {
        let ty = descendant_value(tree, parameter, values)
            .ok_or(TypeBindingError::InvalidSyntax(parameter))?;
        let position = parameters.len();
        parameters.push(ty);
        if let Some(name) = callable_parameter_name(namespaces, tree, parameter)?
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

fn named_segments(
    namespaces: &PreparedNamespaces<'_>,
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
        let _ = namespaces;
        Ok(segments)
    }
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

fn declaration_module(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::ModuleId, TypeBindingError> {
    let reserved = &namespaces.imports.generics.headers.reserved;
    let surface = reserved
        .declarations
        .get(declaration.index())
        .ok_or(TypeBindingError::MissingSource(declaration))?;
    reserved
        .module_for_source(surface.source())
        .ok_or(TypeBindingError::MissingSource(declaration))
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

fn builtin(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    node: NodeId,
) -> Result<nocter_model::BuiltinType, TypeBindingError> {
    let token = tree
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    match token_text(namespaces, tree, token)? {
        "bool" => Ok(nocter_model::BuiltinType::Bool),
        "i8" => Ok(nocter_model::BuiltinType::I8),
        "i16" => Ok(nocter_model::BuiltinType::I16),
        "i32" => Ok(nocter_model::BuiltinType::I32),
        "i64" => Ok(nocter_model::BuiltinType::I64),
        "u8" => Ok(nocter_model::BuiltinType::U8),
        "u16" => Ok(nocter_model::BuiltinType::U16),
        "u32" => Ok(nocter_model::BuiltinType::U32),
        "u64" => Ok(nocter_model::BuiltinType::U64),
        "usize" => Ok(nocter_model::BuiltinType::Usize),
        "isize" => Ok(nocter_model::BuiltinType::Isize),
        "str" => Ok(nocter_model::BuiltinType::Str),
        "error" => Ok(nocter_model::BuiltinType::Error),
        "void" => Ok(nocter_model::BuiltinType::Void),
        "never" => Ok(nocter_model::BuiltinType::Never),
        _ => Err(TypeBindingError::InvalidSyntax(node)),
    }
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

fn token_symbol(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<Symbol, TypeBindingError> {
    let text = token_text(namespaces, tree, token)?;
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .symbols()
        .get(text)
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
}

fn token_text<'a>(
    namespaces: &'a PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<&'a str, TypeBindingError> {
    let source = namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))?;
    source
        .text_at(token.range())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
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

struct NameSegment {
    token: SyntaxToken,
    arguments: Vec<BoundTypeId>,
}

enum NameState {
    Module(nocter_model::ModuleId),
    Type(BoundTypeId),
}
