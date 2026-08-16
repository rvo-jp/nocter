mod capability;
mod context;
mod names;
mod normalization;
mod pattern;
mod projection;
mod requirements;
mod syntax;

use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_declarations::{ExpansionCapability, RequirementSubject};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, GenericParameterId, InterfaceId,
    NominalTypeId, ParameterOrigin, Symbol, TypeAliasId,
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
    named_parameters: Box<[bool]>,
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
    pub const fn named_parameters(&self) -> &[bool] {
        &self.named_parameters
    }

    #[must_use]
    pub fn explicit_origins(&self) -> Option<&[ParameterOrigin]> {
        self.explicit_origins.as_deref()
    }
}

pub use normalization::{
    NormalizedDeclarationPattern, PreparedTypes, TypeNormalizationError, normalize_header_types,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundCapability {
    Interface {
        definition: InterfaceId,
        arguments: Box<[BoundTypeId]>,
    },
    Callable(BoundTypeId),
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

/// A declaration target pattern after its type head and binder occurrences are resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundDeclarationPattern {
    Builtin(BuiltinType),
    Slice(GenericParameterId),
    Nominal {
        definition: NominalTypeId,
        arguments: Box<[GenericParameterId]>,
    },
    Interface {
        definition: InterfaceId,
        arguments: Box<[GenericParameterId]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundRequirementKind {
    Capability {
        subject: RequirementSubject,
        capability: BoundCapability,
    },
    Copy(GenericParameterId),
    TypeEquality {
        left: BoundTypeId,
        right: BoundTypeId,
    },
    Equality {
        operand: GenericParameterId,
    },
    Ordering {
        operand: GenericParameterId,
    },
    Index {
        capability: BorrowCapability,
        container: GenericParameterId,
        index: BoundTypeId,
        result: BoundTypeId,
    },
    Coercion {
        source: BoundTypeId,
        target: BoundTypeId,
    },
    Expansion {
        capability: ExpansionCapability,
        source: GenericParameterId,
        result: BoundTypeId,
    },
    BinderRefinement {
        parameter: GenericParameterId,
        replacement: BoundTypeId,
    },
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
    InvalidRequirement(NodeId),
    RecursiveBinderRefinement(NodeId),
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
            Self::InvalidRequirement(node) => {
                write!(
                    formatter,
                    "generic requirement {node:?} has an invalid semantic shape"
                )
            }
            Self::RecursiveBinderRefinement(node) => write!(
                formatter,
                "binder refinement {node:?} contains the binder it replaces"
            ),
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
    root_declarations: HashMap<NodeId, SurfaceDeclarationId>,
    alias_targets: HashMap<TypeAliasId, BoundTypeId>,
    patterns: Box<[Box<[BoundDeclarationPattern]>]>,
    capabilities: HashMap<NodeId, BoundCapability>,
    capability_declarations: HashMap<NodeId, SurfaceDeclarationId>,
    requirements: Box<[Box<[BoundRequirementKind]>]>,
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
    pub fn declaration_patterns(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<&[BoundDeclarationPattern]> {
        self.patterns.get(declaration.index()).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn capability_for(&self, node: NodeId) -> Option<&BoundCapability> {
        self.capabilities.get(&node)
    }

    #[must_use]
    pub fn declaration_requirements(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<&[BoundRequirementKind]> {
        self.requirements
            .get(declaration.index())
            .map(AsRef::as_ref)
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
    let declaration_nodes = declaration_node_set(&namespaces);
    let mut kinds = Vec::new();
    let mut roots = HashMap::new();
    let mut root_declarations = HashMap::new();
    let mut alias_targets = HashMap::new();
    let mut capabilities = HashMap::new();
    let mut capability_declarations = HashMap::new();
    let declaration_count = namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .len();

    let mut patterns = Vec::with_capacity(declaration_count);
    let mut requirements = Vec::with_capacity(declaration_count);
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
                &mut root_declarations,
            )?;
        }
        record_alias_target(
            &namespaces,
            declaration,
            tree,
            surface.node(),
            &roots,
            &mut alias_targets,
        )?;
        for capability in header_nodes(
            tree,
            surface.node(),
            &declaration_nodes,
            NodeKind::Capability,
        ) {
            let bound = capability::bind(
                &mut namespaces,
                declaration,
                tree,
                capability,
                &mut kinds,
                &mut roots,
                &mut root_declarations,
            )?;
            capabilities.insert(capability, bound);
            capability_declarations.insert(capability, declaration);
        }
        patterns.push(
            pattern::bind_all(&mut namespaces, declaration, tree, surface.node())?
                .into_boxed_slice(),
        );
        requirements.push(
            requirements::bind_all(
                &mut namespaces,
                declaration,
                tree,
                surface.node(),
                &mut kinds,
                &roots,
                &capabilities,
            )?
            .into_boxed_slice(),
        );
    }

    Ok(PreparedTypeBindings {
        namespaces,
        kinds: kinds.into_boxed_slice(),
        roots,
        root_declarations,
        alias_targets,
        patterns: patterns.into_boxed_slice(),
        capabilities,
        capability_declarations,
        requirements: requirements.into_boxed_slice(),
    })
}

fn declaration_node_set(namespaces: &PreparedNamespaces<'_>) -> HashSet<NodeId> {
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .iter()
        .map(|declaration| declaration.node())
        .collect()
}

fn record_alias_target(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
    tree: &nocter_syntax::SyntaxTree,
    declaration_node: NodeId,
    roots: &HashMap<NodeId, BoundTypeId>,
    alias_targets: &mut HashMap<TypeAliasId, BoundTypeId>,
) -> Result<(), TypeBindingError> {
    let Some(ReservedEntity::TypeAlias(alias)) = namespaces
        .imports
        .generics
        .headers
        .reserved
        .entity(declaration)
    else {
        return Ok(());
    };
    let target_node = direct_node(tree, declaration_node, NodeKind::Type)
        .ok_or(TypeBindingError::InvalidSyntax(declaration_node))?;
    let target = roots
        .get(&target_node)
        .copied()
        .ok_or(TypeBindingError::InvalidSyntax(target_node))?;
    alias_targets.insert(alias, target);
    Ok(())
}

fn header_type_roots(
    tree: &nocter_syntax::SyntaxTree,
    declaration: NodeId,
    declaration_nodes: &HashSet<NodeId>,
) -> Vec<NodeId> {
    header_nodes(tree, declaration, declaration_nodes, NodeKind::Type)
}

fn header_nodes(
    tree: &nocter_syntax::SyntaxTree,
    declaration: NodeId,
    declaration_nodes: &HashSet<NodeId>,
    expected: NodeKind,
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
        if syntax.kind() == expected {
            roots.push(node);
            continue;
        }
        pending.extend(tree.children(node).iter().rev().copied());
    }
    roots
}

fn direct_node(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == expected) =>
            {
                Some(*child)
            }
            _ => None,
        })
}

pub(super) fn push(kinds: &mut Vec<BoundTypeKind>, kind: BoundTypeKind) -> BoundTypeId {
    let id = BoundTypeId(kinds.len());
    kinds.push(kind);
    id
}

#[cfg(test)]
mod normalization_tests;
#[cfg(test)]
mod requirement_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
