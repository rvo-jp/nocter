mod projection;
mod syntax;

use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_model::{
    BorrowCapability, CallableCapability, GenericParameterId, NominalTypeId, ParameterOrigin,
    Symbol, TypeAliasId,
};
use nocter_source::SourceId;
use nocter_source_index::DuplicateSourceBinding;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundTypeId(usize);

impl BoundTypeId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A callable type whose names are bound but whose component types are not yet interned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundCallableType {
    capability: CallableCapability,
    parameters: Box<[BoundTypeId]>,
    result: BoundTypeId,
    explicit_origins: Option<Box<[ParameterOrigin]>>,
}

impl BoundCallableType {
    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn parameters(&self) -> &[BoundTypeId] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> BoundTypeId {
        self.result
    }

    #[must_use]
    pub fn explicit_origins(&self) -> Option<&[ParameterOrigin]> {
        self.explicit_origins.as_deref()
    }
}

/// A syntax-independent type expression with every lexical type name bound to semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundTypeKind {
    Builtin(nocter_model::BuiltinType),
    GenericParameter(GenericParameterId),
    SelfType(ReservedEntity),
    Nominal {
        definition: NominalTypeId,
        arguments: Box<[BoundTypeId]>,
    },
    Alias {
        definition: TypeAliasId,
        arguments: Box<[BoundTypeId]>,
    },
    AssociatedSelection {
        base: BoundTypeId,
        name: Symbol,
    },
    Pointer(BoundTypeId),
    Borrow {
        capability: BorrowCapability,
        referent: BoundTypeId,
    },
    Slice(BoundTypeId),
    FixedArray {
        element: BoundTypeId,
        length: u64,
    },
    Callable(BoundCallableType),
    Optional(BoundTypeId),
    Fallible(BoundTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeBindingError {
    MissingSource(SurfaceDeclarationId),
    InvalidSyntax(NodeId),
    UnknownName(NodeId),
    InvalidTypeEntity(NodeId),
    InvalidTypeArguments(NodeId),
    InvalidSelfType(NodeId),
    InvalidArrayLength(NodeId),
    DuplicateCallableParameter(NodeId),
    UnknownProvenanceOrigin(NodeId),
    DuplicateProvenanceOrigin(NodeId),
    InconsistentSource(SourceId),
    DuplicateSourceBinding(DuplicateSourceBinding),
}

impl fmt::Display for TypeBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(declaration) => {
                write!(formatter, "declaration {declaration:?} has no source")
            }
            Self::InvalidSyntax(node) => write!(formatter, "type syntax {node:?} is inconsistent"),
            Self::UnknownName(node) => write!(formatter, "type syntax {node:?} names no type"),
            Self::InvalidTypeEntity(node) => {
                write!(
                    formatter,
                    "type syntax {node:?} resolves to a non-type entity"
                )
            }
            Self::InvalidTypeArguments(node) => {
                write!(formatter, "type syntax {node:?} has invalid type arguments")
            }
            Self::InvalidSelfType(node) => {
                write!(formatter, "Self has no type-owning context at {node:?}")
            }
            Self::InvalidArrayLength(node) => {
                write!(formatter, "fixed-array type {node:?} has an invalid length")
            }
            Self::DuplicateCallableParameter(node) => {
                write!(formatter, "callable type {node:?} repeats a parameter name")
            }
            Self::UnknownProvenanceOrigin(node) => {
                write!(
                    formatter,
                    "callable type {node:?} names an unknown result origin"
                )
            }
            Self::DuplicateProvenanceOrigin(node) => {
                write!(formatter, "callable type {node:?} repeats a result origin")
            }
            Self::InconsistentSource(source) => {
                write!(formatter, "{source} has an inconsistent type origin")
            }
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypeBindingError {}

impl From<DuplicateSourceBinding> for TypeBindingError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

/// Complete header type syntax after lexical names and callable origins are bound.
#[derive(Debug)]
pub struct PreparedTypeBindings<'syntax> {
    namespaces: PreparedNamespaces<'syntax>,
    kinds: Box<[BoundTypeKind]>,
    roots: HashMap<NodeId, BoundTypeId>,
}

impl PreparedTypeBindings<'_> {
    #[must_use]
    pub const fn namespaces(&self) -> &PreparedNamespaces<'_> {
        &self.namespaces
    }

    #[must_use]
    pub fn kind(&self, id: BoundTypeId) -> Option<&BoundTypeKind> {
        self.kinds.get(id.index())
    }

    #[must_use]
    pub fn type_for(&self, node: NodeId) -> Option<BoundTypeId> {
        self.roots.get(&node).copied()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.kinds.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

/// Binds every declaration-header `Type` node without normalizing aliases or associated types.
///
/// Blocks and nested member declarations are separate semantic scopes and are not traversed from
/// their parent. Each member is processed through its own surface identity. The resulting flat
/// arena is syntax-independent; syntax IDs remain only in this temporary lowering-stage index.
///
/// # Errors
///
/// Returns [`TypeBindingError`] for unknown or non-type names, inaccessible selections, invalid
/// arity, invalid `Self`, malformed fixed-array lengths or callable provenance, inconsistent source
/// projection, or duplicate source bindings.
pub fn bind_header_type_syntax(
    mut namespaces: PreparedNamespaces<'_>,
) -> Result<PreparedTypeBindings<'_>, TypeBindingError> {
    let declaration_nodes: HashSet<_> = namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .iter()
        .map(|declaration| declaration.node())
        .collect();
    let mut kinds = Vec::new();
    let mut roots = HashMap::new();
    let declaration_count = namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .len();

    for index in 0..declaration_count {
        let declaration = SurfaceDeclarationId::from_index(index);
        let surface = namespaces.imports.generics.headers.reserved.declarations[index];
        let tree = namespaces
            .imports
            .generics
            .headers
            .reserved
            .sources
            .get(surface.source().index())
            .ok_or(TypeBindingError::MissingSource(declaration))?
            .syntax();
        let type_roots = header_type_roots(tree, surface.node(), &declaration_nodes);
        for root in type_roots {
            if roots.contains_key(&root) {
                continue;
            }
            syntax::bind(
                &mut namespaces,
                declaration,
                tree,
                root,
                &mut kinds,
                &mut roots,
            )?;
        }
    }

    Ok(PreparedTypeBindings {
        namespaces,
        kinds: kinds.into_boxed_slice(),
        roots,
    })
}

fn header_type_roots(
    tree: &nocter_syntax::SyntaxTree,
    declaration: NodeId,
    declaration_nodes: &HashSet<NodeId>,
) -> Vec<NodeId> {
    let mut roots = Vec::new();
    let mut pending: Vec<_> = tree.children(declaration).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let Some(syntax) = tree.node(node) else {
            continue;
        };
        if syntax.kind() == NodeKind::Block || declaration_nodes.contains(&node) {
            continue;
        }
        if syntax.kind() == NodeKind::Type {
            roots.push(node);
            continue;
        }
        pending.extend(tree.children(node).iter().rev().copied());
    }
    roots
}

pub(super) fn push(kinds: &mut Vec<BoundTypeKind>, kind: BoundTypeKind) -> BoundTypeId {
    let id = BoundTypeId(kinds.len());
    kinds.push(kind);
    id
}

#[cfg(test)]
mod tests;
