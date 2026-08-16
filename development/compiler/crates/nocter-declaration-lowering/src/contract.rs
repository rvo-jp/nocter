use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_syntax::{NodeId, NodeKind, SyntaxElement, TokenKind};

use crate::{
    DeclarationSurface, ModuleIdentity, ModuleSourceKind, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HeaderFingerprint(Box<[Box<str>]>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CallableLabel {
    Named(Box<str>),
    LiteralSequence,
    LiteralString,
    Coercion(HeaderFingerprint),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CallableKey {
    module: ModuleIdentity,
    kind: SurfaceDeclarationKind,
    owner: Option<HeaderFingerprint>,
    header: HeaderFingerprint,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LooseCallableKey {
    module: ModuleIdentity,
    kind: SurfaceDeclarationKind,
    owner: Option<HeaderFingerprint>,
    label: CallableLabel,
}

/// The canonical semantic representative selected for every authored declaration surface.
///
/// A public bodyless callable and its private implementation body share the public contract's
/// representative. All other declarations initially represent themselves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableContracts {
    representatives: Box<[SurfaceDeclarationId]>,
}

impl CallableContracts {
    #[must_use]
    pub fn representative(&self, declaration: SurfaceDeclarationId) -> SurfaceDeclarationId {
        self.representatives[declaration.index()]
    }

    #[must_use]
    pub fn is_implementation(&self, declaration: SurfaceDeclarationId) -> bool {
        self.representative(declaration) != declaration
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableContractError {
    MissingBody(NodeId),
    MismatchedBody { contract: NodeId, body: NodeId },
    DuplicateBody { contract: NodeId, body: NodeId },
    InvalidBodyOmission(NodeId),
    UnmatchedImplementationEntry(NodeId),
    InconsistentSurface(NodeId),
}

impl fmt::Display for CallableContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBody(contract) => {
                write!(
                    formatter,
                    "public callable contract {contract:?} has no body"
                )
            }
            Self::MismatchedBody { contract, body } => write!(
                formatter,
                "implementation body {body:?} does not match contract {contract:?}"
            ),
            Self::DuplicateBody { contract, body } => write!(
                formatter,
                "callable contract {contract:?} has duplicate implementation body {body:?}"
            ),
            Self::InvalidBodyOmission(node) => write!(
                formatter,
                "callable {node:?} omits its body outside an eligible public root contract"
            ),
            Self::UnmatchedImplementationEntry(node) => write!(
                formatter,
                "root-surface-only implementation entry {node:?} has no public contract"
            ),
            Self::InconsistentSurface(node) => {
                write!(
                    formatter,
                    "declaration surface around {node:?} is inconsistent"
                )
            }
        }
    }
}

impl std::error::Error for CallableContractError {}

/// Joins eligible public root contracts to exact private implementation bodies.
///
/// Header equality uses canonical token spelling: newlines, visibility, bodies, and construction
/// `default` are excluded, and all remaining authored header tokens are compared exactly. No name
/// or type is resolved during this pass.
///
/// # Errors
///
/// Returns [`CallableContractError`] for a missing, mismatched, duplicate, or ineligible body.
pub fn analyze_callable_contracts(
    surface: &DeclarationSurface<'_>,
) -> Result<CallableContracts, CallableContractError> {
    let count = surface.declarations().len();
    let mut representatives: Vec<_> = (0..count).map(SurfaceDeclarationId::from_index).collect();
    let candidates = collect_body_candidates(surface)?;
    let joined = join_contracts(surface, &candidates, &mut representatives)?;
    reject_unmatched_entries(surface, &joined.used)?;
    join_implementation_containers(
        surface,
        &joined.used,
        joined.container_targets,
        &mut representatives,
    )?;

    Ok(CallableContracts {
        representatives: representatives.into_boxed_slice(),
    })
}

struct BodyCandidates {
    exact: BTreeMap<CallableKey, Vec<SurfaceDeclarationId>>,
    loose: BTreeMap<LooseCallableKey, Vec<SurfaceDeclarationId>>,
}

struct JoinedBodies {
    used: BTreeSet<SurfaceDeclarationId>,
    container_targets: BTreeMap<SurfaceDeclarationId, BTreeSet<SurfaceDeclarationId>>,
}

fn collect_body_candidates(
    surface: &DeclarationSurface<'_>,
) -> Result<BodyCandidates, CallableContractError> {
    let mut exact_bodies: BTreeMap<CallableKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    let mut loose_bodies: BTreeMap<LooseCallableKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if !is_separable_callable(declaration.kind())
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        if !has_body(surface, declaration)? {
            return Err(CallableContractError::InvalidBodyOmission(
                declaration.node(),
            ));
        }
        let (exact, loose) = callable_keys(surface, declaration)?;
        exact_bodies.entry(exact).or_default().push(id);
        loose_bodies.entry(loose).or_default().push(id);
    }
    Ok(BodyCandidates {
        exact: exact_bodies,
        loose: loose_bodies,
    })
}

fn join_contracts(
    surface: &DeclarationSurface<'_>,
    candidates: &BodyCandidates,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<JoinedBodies, CallableContractError> {
    let mut used_bodies = BTreeSet::new();
    let mut container_targets: BTreeMap<SurfaceDeclarationId, BTreeSet<SurfaceDeclarationId>> =
        BTreeMap::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if !is_separable_callable(contract.kind()) || has_body(surface, contract)? {
            continue;
        }
        if source_kind(surface, contract)? != ModuleSourceKind::Root
            || contract.visibility().is_none()
        {
            return Err(CallableContractError::InvalidBodyOmission(contract.node()));
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let (exact, loose) = callable_keys(surface, contract)?;
        match candidates
            .exact
            .get(&exact)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            [] => {
                if let Some(body) = candidates
                    .loose
                    .get(&loose)
                    .and_then(|bodies| bodies.first())
                {
                    return Err(CallableContractError::MismatchedBody {
                        contract: contract.node(),
                        body: surface.declarations()[body.index()].node(),
                    });
                }
                return Err(CallableContractError::MissingBody(contract.node()));
            }
            [body] => {
                if !used_bodies.insert(*body) {
                    return Err(CallableContractError::DuplicateBody {
                        contract: contract.node(),
                        body: surface.declarations()[body.index()].node(),
                    });
                }
                representatives[body.index()] = contract_id;
                if let (Some(body_owner), Some(contract_owner)) = (
                    surface.declarations()[body.index()].owner(),
                    contract.owner(),
                ) {
                    container_targets
                        .entry(body_owner)
                        .or_default()
                        .insert(contract_owner);
                }
            }
            [_, second, ..] => {
                return Err(CallableContractError::DuplicateBody {
                    contract: contract.node(),
                    body: surface.declarations()[second.index()].node(),
                });
            }
        }
    }
    Ok(JoinedBodies {
        used: used_bodies,
        container_targets,
    })
}

fn reject_unmatched_entries(
    surface: &DeclarationSurface<'_>,
    used_bodies: &BTreeSet<SurfaceDeclarationId>,
) -> Result<(), CallableContractError> {
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if source_kind(surface, declaration)? == ModuleSourceKind::Implementation
            && matches!(
                declaration.kind(),
                SurfaceDeclarationKind::ConstructionFunction
                    | SurfaceDeclarationKind::Literal
                    | SurfaceDeclarationKind::Coercion
            )
            && !used_bodies.contains(&id)
        {
            return Err(CallableContractError::UnmatchedImplementationEntry(
                declaration.node(),
            ));
        }
    }
    Ok(())
}

fn join_implementation_containers(
    surface: &DeclarationSurface<'_>,
    used_bodies: &BTreeSet<SurfaceDeclarationId>,
    container_targets: BTreeMap<SurfaceDeclarationId, BTreeSet<SurfaceDeclarationId>>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), CallableContractError> {
    for (implementation_owner, targets) in container_targets {
        if targets.len() != 1 {
            continue;
        }
        let all_members_are_joined = surface
            .declarations()
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, declaration)| declaration.owner() == Some(implementation_owner))
            .all(|(index, _)| used_bodies.contains(&SurfaceDeclarationId::from_index(index)));
        if all_members_are_joined {
            representatives[implementation_owner.index()] =
                *targets
                    .first()
                    .ok_or(CallableContractError::InconsistentSurface(
                        surface.declarations()[implementation_owner.index()].node(),
                    ))?;
        }
    }
    Ok(())
}

fn callable_keys(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<(CallableKey, LooseCallableKey), CallableContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        CallableContractError::InconsistentSurface(declaration.node()),
    )?;
    let header = fingerprint(surface, declaration)?;
    let owner = declaration
        .owner()
        .map(|owner| {
            let owner = surface.declarations().get(owner.index()).copied().ok_or(
                CallableContractError::InconsistentSurface(declaration.node()),
            )?;
            fingerprint(surface, owner)
        })
        .transpose()?;
    let label = callable_label(declaration.kind(), &header).ok_or(
        CallableContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok((
        CallableKey {
            module: source.module().clone(),
            kind: declaration.kind(),
            owner: owner.clone(),
            header: header.clone(),
        },
        LooseCallableKey {
            module: source.module().clone(),
            kind: declaration.kind(),
            owner,
            label,
        },
    ))
}

fn fingerprint(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<HeaderFingerprint, CallableContractError> {
    let surface_source = surface.sources().get(declaration.source().index()).ok_or(
        CallableContractError::InconsistentSurface(declaration.node()),
    )?;
    let tree = surface_source.syntax();
    let source = surface.source_map().get(tree.source()).ok_or(
        CallableContractError::InconsistentSurface(declaration.node()),
    )?;
    tree.node(declaration.node())
        .ok_or(CallableContractError::InconsistentSurface(
            declaration.node(),
        ))?;
    let mut pending: Vec<_> = tree
        .children(declaration.node())
        .iter()
        .rev()
        .map(|element| (*element, 0_usize))
        .collect();
    let mut tokens = Vec::new();
    while let Some((element, depth)) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => {
                let kind = tree
                    .node(node)
                    .ok_or(CallableContractError::InconsistentSurface(
                        declaration.node(),
                    ))?
                    .kind();
                if kind == NodeKind::Visibility
                    || kind == NodeKind::Block
                    || is_member_declaration(kind)
                {
                    continue;
                }
                pending.extend(
                    tree.children(node)
                        .iter()
                        .rev()
                        .map(|element| (*element, depth + 1)),
                );
            }
            SyntaxElement::Token(token) => {
                if matches!(token.kind(), TokenKind::Newline | TokenKind::Eof) {
                    continue;
                }
                let spelling = source.text_at(token.range()).ok_or(
                    CallableContractError::InconsistentSurface(declaration.node()),
                )?;
                if depth == 0
                    && matches!(
                        declaration.kind(),
                        SurfaceDeclarationKind::ConstructionFunction
                            | SurfaceDeclarationKind::Literal
                    )
                    && spelling == "default"
                {
                    continue;
                }
                tokens.push(Box::<str>::from(spelling));
            }
            SyntaxElement::Missing(_) => {
                return Err(CallableContractError::InconsistentSurface(
                    declaration.node(),
                ));
            }
        }
    }
    Ok(HeaderFingerprint(tokens.into_boxed_slice()))
}

fn callable_label(
    kind: SurfaceDeclarationKind,
    header: &HeaderFingerprint,
) -> Option<CallableLabel> {
    let tokens = &header.0;
    match kind {
        SurfaceDeclarationKind::Function | SurfaceDeclarationKind::ConstructionFunction => tokens
            .iter()
            .position(|token| token.as_ref() == "func")
            .and_then(|index| tokens.get(index + 1))
            .cloned()
            .map(CallableLabel::Named),
        SurfaceDeclarationKind::InherentMethod => tokens
            .iter()
            .position(|token| token.as_ref() == ".")
            .and_then(|index| tokens.get(index + 1))
            .cloned()
            .map(CallableLabel::Named),
        SurfaceDeclarationKind::Literal => {
            if tokens.iter().any(|token| token.as_ref() == "[") {
                Some(CallableLabel::LiteralSequence)
            } else {
                Some(CallableLabel::LiteralString)
            }
        }
        SurfaceDeclarationKind::Coercion => {
            let end = tokens
                .iter()
                .position(|token| token.as_ref() == "from")
                .unwrap_or(tokens.len());
            Some(CallableLabel::Coercion(HeaderFingerprint(
                tokens[..end].into(),
            )))
        }
        _ => None,
    }
}

fn source_kind(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<ModuleSourceKind, CallableContractError> {
    surface
        .sources()
        .get(declaration.source().index())
        .map(crate::SurfaceSource::kind)
        .ok_or(CallableContractError::InconsistentSurface(
            declaration.node(),
        ))
}

fn has_body(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<bool, CallableContractError> {
    let tree = surface
        .sources()
        .get(declaration.source().index())
        .ok_or(CallableContractError::InconsistentSurface(
            declaration.node(),
        ))?
        .syntax();
    let mut pending = vec![declaration.node()];
    while let Some(node) = pending.pop() {
        for child in tree.children(node) {
            if let SyntaxElement::Node(child) = child {
                let kind = tree
                    .node(*child)
                    .ok_or(CallableContractError::InconsistentSurface(
                        declaration.node(),
                    ))?
                    .kind();
                if kind == NodeKind::Block {
                    return Ok(true);
                }
                if !is_member_declaration(kind) {
                    pending.push(*child);
                }
            }
        }
    }
    Ok(false)
}

const fn is_separable_callable(kind: SurfaceDeclarationKind) -> bool {
    matches!(
        kind,
        SurfaceDeclarationKind::Function
            | SurfaceDeclarationKind::InherentMethod
            | SurfaceDeclarationKind::ConstructionFunction
            | SurfaceDeclarationKind::Literal
            | SurfaceDeclarationKind::Coercion
    )
}

const fn is_member_declaration(kind: NodeKind) -> bool {
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
            | NodeKind::ConformMethod
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
    )
}

#[cfg(test)]
mod tests;
