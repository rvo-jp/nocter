#[path = "generics/violation.rs"]
mod violation;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{GenericOwner, GenericParameter};
use nocter_model::{GenericParameterId, Symbol};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind, direct_node_iter,
    direct_token,
};

use crate::{
    PreparedHeaders, ReservedEntity, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

pub use violation::{GenericRule, GenericViolation};

#[derive(Clone, Copy, Debug)]
struct GenericBinding {
    name: Symbol,
    parameter: GenericParameterId,
    origin: SyntaxToken,
}

type GenericScope = Box<[GenericBinding]>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenericError {
    Rule(GenericViolation),
    MissingSource(SurfaceDeclarationId),
    InconsistentSource(SurfaceDeclarationId),
    InconsistentBinder(SurfaceDeclarationId),
    InvalidOwner(SurfaceDeclarationId),
    InconsistentContract(SurfaceDeclarationId),
}

impl fmt::Display for GenericError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::MissingSource(declaration) => {
                write!(formatter, "declaration {declaration:?} has no source")
            }
            Self::InconsistentSource(declaration) => write!(
                formatter,
                "declaration {declaration:?} has an inconsistent generic binder origin"
            ),
            Self::InconsistentBinder(declaration) => write!(
                formatter,
                "declaration {declaration:?} has an inconsistent generic binder"
            ),
            Self::InvalidOwner(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} cannot own generic binders"
                )
            }
            Self::InconsistentContract(declaration) => write!(
                formatter,
                "implementation declaration {declaration:?} changed its generic binders"
            ),
        }
    }
}

impl std::error::Error for GenericError {}

impl From<GenericViolation> for GenericError {
    fn from(violation: GenericViolation) -> Self {
        Self::Rule(violation)
    }
}

/// Declaration headers with generic identities and complete lexical generic scopes.
#[derive(Debug)]
pub struct PreparedGenerics<'syntax> {
    pub(crate) headers: PreparedHeaders<'syntax>,
    pub(crate) own: Box<[Box<[GenericParameterId]>]>,
    visible: Box<[GenericScope]>,
}

impl PreparedGenerics<'_> {
    #[must_use]
    pub const fn headers(&self) -> &PreparedHeaders<'_> {
        &self.headers
    }

    #[must_use]
    pub fn own(&self, declaration: SurfaceDeclarationId) -> Option<&[GenericParameterId]> {
        self.own.get(declaration.index()).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn lookup(
        &self,
        declaration: SurfaceDeclarationId,
        name: Symbol,
    ) -> Option<GenericParameterId> {
        self.visible
            .get(declaration.index())?
            .binary_search_by_key(&name, |binding| binding.name)
            .ok()
            .map(|index| self.visible[declaration.index()][index].parameter)
    }

    pub(crate) fn visible_ids(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<Vec<GenericParameterId>> {
        Some(
            self.visible
                .get(declaration.index())?
                .iter()
                .map(|binding| binding.parameter)
                .collect(),
        )
    }
}

/// Allocates generic parameters after all recursive declaration identities exist.
///
/// # Errors
///
/// Returns [`GenericError`] for invalid, duplicate, shadowing, or contract-inconsistent binders,
/// invalid owners, missing syntax, or duplicate source projection.
pub fn prepare_generic_binders(
    mut headers: PreparedHeaders<'_>,
) -> Result<PreparedGenerics<'_>, GenericError> {
    let count = headers.reserved.declarations.len();
    let mut own = vec![Box::<[GenericParameterId]>::default(); count];
    let mut visible = vec![GenericScope::default(); count];

    for index in 0..count {
        let id = SurfaceDeclarationId::from_index(index);
        let declaration = headers.reserved.declarations[index];
        let representative = headers.reserved.contracts.representative(id);
        if representative != id {
            if representative.index() >= index {
                return Err(GenericError::InconsistentContract(id));
            }
            let (previous_own, current_own) = own.split_at_mut(index);
            current_own[0].clone_from(&previous_own[representative.index()]);
            let (previous_visible, current_visible) = visible.split_at_mut(index);
            current_visible[0].clone_from(&previous_visible[representative.index()]);
            project_implementation_binders(&mut headers, id, &own[index])?;
            continue;
        }
        let inherited = inherited_scope(&headers, &visible, id, declaration)?;
        let binders = binder_tokens(&headers, id, declaration)?;
        let owner = generic_owner(&headers, id, binders.tokens.is_empty())?;
        let mut scope: BTreeMap<_, _> = inherited
            .iter()
            .copied()
            .map(|binding| (binding.name, binding))
            .collect();
        let mut local = BTreeMap::new();
        let mut ids = Vec::new();
        for token in binders.tokens {
            let name = binder_symbol(&headers, id, token)?;
            if let Some((parameter, first)) = local.get(&name).copied() {
                if !binders.reuses_local_names {
                    return Err(GenericViolation::duplicate_binder(first, token).into());
                }
                project_binder(&mut headers, id, parameter, token, SourceRole::Reference)?;
                continue;
            }
            if let Ok(index) = inherited.binary_search_by_key(&name, |binding| binding.name) {
                return Err(
                    GenericViolation::shadowing_binder(inherited[index].origin, token).into(),
                );
            }
            let owner = owner.ok_or(GenericError::InvalidOwner(id))?;
            let parameter = headers
                .reserved
                .program
                .declarations_mut()
                .add_generic_parameter(GenericParameter::new(owner, name, ids.len()));
            scope.insert(
                name,
                GenericBinding {
                    name,
                    parameter,
                    origin: token,
                },
            );
            local.insert(name, (parameter, token));
            ids.push(parameter);
            project_binder(&mut headers, id, parameter, token, SourceRole::Declaration)?;
        }
        own[index] = ids.into_boxed_slice();
        visible[index] = scope.into_values().collect::<Vec<_>>().into_boxed_slice();
    }

    Ok(PreparedGenerics {
        headers,
        own: own.into_boxed_slice(),
        visible: visible.into_boxed_slice(),
    })
}

fn inherited_scope(
    headers: &PreparedHeaders<'_>,
    visible: &[GenericScope],
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<GenericScope, GenericError> {
    let Some(owner) = declaration.owner() else {
        return Ok(Box::new([]));
    };
    visible
        .get(owner.index())
        .cloned()
        .ok_or(GenericError::InvalidOwner(id))
        .and_then(|scope| {
            if headers.reserved.declarations.get(owner.index()).is_some() {
                Ok(scope)
            } else {
                Err(GenericError::InvalidOwner(id))
            }
        })
}

fn generic_owner(
    headers: &PreparedHeaders<'_>,
    id: SurfaceDeclarationId,
    empty: bool,
) -> Result<Option<GenericOwner>, GenericError> {
    let entity = headers.reserved.entity(id);
    let owner = match entity {
        Some(ReservedEntity::NominalType(owner)) => Some(GenericOwner::NominalType(owner)),
        Some(ReservedEntity::TypeAlias(owner)) => Some(GenericOwner::TypeAlias(owner)),
        Some(ReservedEntity::Interface(owner)) => Some(GenericOwner::Interface(owner)),
        Some(ReservedEntity::Callable(owner)) => Some(GenericOwner::Callable(owner)),
        Some(ReservedEntity::Construction(owner)) => Some(GenericOwner::Construction(owner)),
        Some(ReservedEntity::Instance(owner)) => Some(GenericOwner::Instance(owner)),
        Some(ReservedEntity::Drop(owner)) => Some(GenericOwner::Drop(owner)),
        _ if empty => None,
        _ => return Err(GenericError::InvalidOwner(id)),
    };
    Ok(owner)
}

#[derive(Debug)]
struct BinderTokens {
    tokens: Vec<SyntaxToken>,
    reuses_local_names: bool,
}

fn binder_tokens(
    headers: &PreparedHeaders<'_>,
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<BinderTokens, GenericError> {
    let tree = headers
        .reserved
        .sources
        .get(declaration.source().index())
        .ok_or(GenericError::MissingSource(id))?
        .syntax();
    match declaration.kind() {
        SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::Drop => Ok(BinderTokens {
            tokens: pattern_binders(tree, declaration.node()),
            reuses_local_names: true,
        }),
        _ => Ok(BinderTokens {
            tokens: find_descendant(tree, declaration.node(), NodeKind::GenericParameters)
                .map(|node| descendant_identifiers(tree, node))
                .unwrap_or_default(),
            reuses_local_names: false,
        }),
    }
}

fn pattern_binders(tree: &nocter_syntax::SyntaxTree, declaration: NodeId) -> Vec<SyntaxToken> {
    let mut binders = Vec::new();
    for pattern in direct_node_iter(tree, declaration, NodeKind::DeclarationTypePattern) {
        if let Some(arguments) = find_descendant(tree, pattern, NodeKind::PatternArguments) {
            binders.extend(descendant_identifiers(tree, arguments));
        } else if direct_token(
            tree,
            pattern,
            TokenKind::Punctuation(Punctuation::LeftBracket),
        )
        .is_some()
            && let Some(token) = descendant_identifiers(tree, pattern).into_iter().next()
        {
            binders.push(token);
        }
    }
    binders
}

fn binder_symbol(
    headers: &PreparedHeaders<'_>,
    id: SurfaceDeclarationId,
    token: SyntaxToken,
) -> Result<Symbol, GenericError> {
    let source = headers
        .reserved
        .source_map
        .get(token.source())
        .ok_or(GenericError::MissingSource(id))?;
    let spelling = source
        .text_at(token.range())
        .ok_or(GenericError::MissingSource(id))?;
    if spelling == "Self" || nocter_syntax::BuiltinType::from_spelling(spelling).is_some() {
        return Err(GenericViolation::reserved_binder(token).into());
    }
    headers
        .reserved
        .program
        .symbols()
        .get(spelling)
        .ok_or(GenericError::InconsistentBinder(id))
}

fn project_binder(
    headers: &mut PreparedHeaders<'_>,
    declaration: SurfaceDeclarationId,
    parameter: GenericParameterId,
    token: SyntaxToken,
    role: SourceRole,
) -> Result<(), GenericError> {
    let declaration_surface = headers
        .reserved
        .declarations
        .get(declaration.index())
        .copied()
        .ok_or(GenericError::InconsistentSource(declaration))?;
    let source = headers
        .reserved
        .sources
        .get(declaration_surface.source().index())
        .ok_or(GenericError::MissingSource(declaration))?;
    headers.reserved.source_index.insert(
        SemanticEntity::GenericParameter(parameter),
        role,
        SourceOrigin::from_token(source.syntax(), token)
            .map_err(|_| GenericError::InconsistentSource(declaration))?,
    );
    Ok(())
}

fn project_implementation_binders(
    headers: &mut PreparedHeaders<'_>,
    id: SurfaceDeclarationId,
    parameters: &[GenericParameterId],
) -> Result<(), GenericError> {
    let declaration = headers.reserved.declarations[id.index()];
    let binders = binder_tokens(headers, id, declaration)?;
    let representative = headers.reserved.contracts.representative(id);
    let representative_declaration = headers.reserved.declarations[representative.index()];
    let representative_binders =
        binder_tokens(headers, representative, representative_declaration)?;
    if binders.reuses_local_names != representative_binders.reuses_local_names {
        return Err(GenericError::InconsistentContract(id));
    }
    if binders.reuses_local_names {
        return project_implementation_pattern_binders(
            headers,
            id,
            representative,
            &binders.tokens,
            &representative_binders.tokens,
            parameters,
        );
    }
    if binders.tokens.len() != parameters.len()
        || representative_binders.tokens.len() != parameters.len()
    {
        return Err(GenericError::InconsistentContract(id));
    }
    for ((parameter, token), representative_token) in parameters
        .iter()
        .copied()
        .zip(binders.tokens)
        .zip(representative_binders.tokens)
    {
        if binder_symbol(headers, id, token)?
            != binder_symbol(headers, representative, representative_token)?
        {
            return Err(GenericError::InconsistentContract(id));
        }
        project_binder(headers, id, parameter, token, SourceRole::Implementation)?;
    }
    Ok(())
}

fn project_implementation_pattern_binders(
    headers: &mut PreparedHeaders<'_>,
    id: SurfaceDeclarationId,
    representative: SurfaceDeclarationId,
    binders: &[SyntaxToken],
    representative_binders: &[SyntaxToken],
    parameters: &[GenericParameterId],
) -> Result<(), GenericError> {
    let mut parameter_by_name = BTreeMap::new();
    let mut parameters = parameters.iter().copied();
    for token in representative_binders {
        let name = binder_symbol(headers, representative, *token)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = parameter_by_name.entry(name) {
            entry.insert(
                parameters
                    .next()
                    .ok_or(GenericError::InconsistentContract(id))?,
            );
        }
    }
    if parameters.next().is_some() {
        return Err(GenericError::InconsistentContract(id));
    }

    let mut projected = BTreeSet::new();
    for token in binders {
        let name = binder_symbol(headers, id, *token)?;
        let parameter = *parameter_by_name
            .get(&name)
            .ok_or(GenericError::InconsistentContract(id))?;
        let role = if projected.insert(name) {
            SourceRole::Implementation
        } else {
            SourceRole::Reference
        };
        project_binder(headers, id, parameter, *token, role)?;
    }
    if projected.len() != parameter_by_name.len() {
        return Err(GenericError::InconsistentContract(id));
    }
    Ok(())
}

fn find_descendant(
    tree: &nocter_syntax::SyntaxTree,
    root: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.node(node)?.kind() == expected {
            return Some(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                let kind = tree.node(*child)?.kind();
                if kind != NodeKind::Block && !is_member(kind) {
                    pending.push(*child);
                }
            }
        }
    }
    None
}

fn descendant_identifiers(tree: &nocter_syntax::SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    nocter_syntax::descendant_identifier_iter(tree, node).collect()
}

const fn is_member(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StructField
            | NodeKind::EnumVariant
            | NodeKind::AssociatedTypeDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::InterfaceImplementation
    )
}

#[cfg(test)]
mod tests;
