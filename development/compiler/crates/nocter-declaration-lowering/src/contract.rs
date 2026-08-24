use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_syntax::{NodeId, NodeKind, SyntaxElement, TokenKind};

use crate::{
    DeclarationSurface, ModuleIdentity, ModuleSourceKind, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

mod constant;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HeaderFingerprint(pub(super) Box<[Box<str>]>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CallableLabel {
    Named(Box<str>),
    LiteralSequence,
    LiteralString,
    Coercion(HeaderFingerprint),
    Operator,
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
pub struct DeclarationContracts {
    representatives: Box<[SurfaceDeclarationId]>,
    representations: Box<[Option<SurfaceDeclarationId>]>,
}

impl DeclarationContracts {
    #[must_use]
    pub fn representative(&self, declaration: SurfaceDeclarationId) -> SurfaceDeclarationId {
        self.representatives[declaration.index()]
    }

    #[must_use]
    pub fn is_implementation(&self, declaration: SurfaceDeclarationId) -> bool {
        self.representative(declaration) != declaration
    }

    /// Returns the private representation occurrence completing a public nominal contract.
    #[must_use]
    pub fn representation(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<SurfaceDeclarationId> {
        self.representations[declaration.index()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationContractError {
    MissingConstantInitializer(NodeId),
    MismatchedConstantInitializer {
        contract: NodeId,
        definition: NodeId,
    },
    DuplicateConstantInitializer {
        contract: NodeId,
        definition: NodeId,
    },
    InvalidConstantOmission(NodeId),
    MissingBody(NodeId),
    MismatchedBody {
        contract: NodeId,
        body: NodeId,
    },
    DuplicateBody {
        contract: NodeId,
        body: NodeId,
    },
    InvalidBodyOmission(NodeId),
    MissingRepresentation(NodeId),
    MismatchedRepresentation {
        contract: NodeId,
        definition: NodeId,
    },
    DuplicateRepresentation {
        contract: NodeId,
        definition: NodeId,
    },
    RepresentationCompletedAgain {
        contract: NodeId,
        definition: NodeId,
    },
    UncontractedConformance(NodeId),
    DuplicateConformanceDefinition {
        contract: NodeId,
        definition: NodeId,
    },
    AmbiguousConformanceContract {
        contract: NodeId,
        conflicting: NodeId,
    },
    InvalidConformanceSplit(NodeId),
    UncontractedInterfaceDefault(NodeId),
    InconsistentSurface(NodeId),
}

impl fmt::Display for DeclarationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConstantInitializer(contract) => write!(
                formatter,
                "constant contract {contract:?} has no initializer definition"
            ),
            Self::MismatchedConstantInitializer {
                contract,
                definition,
            } => write!(
                formatter,
                "constant initializer {definition:?} does not match contract {contract:?}"
            ),
            Self::DuplicateConstantInitializer {
                contract,
                definition,
            } => write!(
                formatter,
                "constant contract {contract:?} has duplicate initializer {definition:?}"
            ),
            Self::InvalidConstantOmission(node) => write!(
                formatter,
                "constant {node:?} omits its initializer outside an eligible public root contract"
            ),
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
            Self::MissingRepresentation(contract) => write!(
                formatter,
                "public nominal contract {contract:?} has no representation definition"
            ),
            Self::MismatchedRepresentation {
                contract,
                definition,
            } => write!(
                formatter,
                "nominal representation {definition:?} does not match contract {contract:?}"
            ),
            Self::DuplicateRepresentation {
                contract,
                definition,
            } => write!(
                formatter,
                "nominal contract {contract:?} has duplicate representation {definition:?}"
            ),
            Self::RepresentationCompletedAgain {
                contract,
                definition,
            } => write!(
                formatter,
                "represented nominal {contract:?} is completed again by {definition:?}"
            ),
            Self::UncontractedConformance(node) => write!(
                formatter,
                "implementation conformance {node:?} has no public index contract"
            ),
            Self::DuplicateConformanceDefinition {
                contract,
                definition,
            } => write!(
                formatter,
                "conformance contract {contract:?} has duplicate implementation definition {definition:?}"
            ),
            Self::AmbiguousConformanceContract {
                contract,
                conflicting,
            } => write!(
                formatter,
                "conformance contract {contract:?} conflicts with duplicate contract {conflicting:?}"
            ),
            Self::InvalidConformanceSplit(node) => write!(
                formatter,
                "separated conformance member {node:?} belongs on the other side of the contract boundary"
            ),
            Self::UncontractedInterfaceDefault(node) => write!(
                formatter,
                "interface default implementation {node:?} has no public index contract"
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

impl std::error::Error for DeclarationContractError {}

/// Joins eligible public root contracts to exact private implementation bodies.
///
/// Header equality uses canonical token spelling: newlines, visibility, and bodies are excluded;
/// construction `default` remains part of the contract. No name or type is resolved during this
/// pass.
///
/// # Errors
///
/// Returns [`DeclarationContractError`] for a missing, mismatched, duplicate, or ineligible body.
pub fn analyze_declaration_contracts(
    surface: &DeclarationSurface<'_>,
) -> Result<DeclarationContracts, DeclarationContractError> {
    let count = surface.declarations().len();
    let mut representatives: Vec<_> = (0..count).map(SurfaceDeclarationId::from_index).collect();
    let mut representations = vec![None; count];
    crate::representation_contract::join_nominal_contracts(
        surface,
        &mut representatives,
        &mut representations,
    )?;
    constant::join(surface, &mut representatives)?;
    join_conformance_contracts(surface, &mut representatives)?;
    let candidates = collect_body_candidates(surface)?;
    let joined = join_contracts(surface, &candidates, &mut representatives)?;
    join_implementation_containers(
        surface,
        &joined.used,
        joined.container_targets,
        &mut representatives,
    )?;

    Ok(DeclarationContracts {
        representatives: representatives.into_boxed_slice(),
        representations: representations.into_boxed_slice(),
    })
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConformanceKey {
    module: ModuleIdentity,
    header: HeaderFingerprint,
}

/// Joins a public conformance fact to its one private implementation container.
///
/// The interface remains the sole owner of required method signatures. A root conformance owns
/// only the conformance head and associated type bindings, so method declarations are deliberately
/// absent from this join key.
fn join_conformance_contracts(
    surface: &DeclarationSurface<'_>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let mut roots: BTreeMap<ConformanceKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != SurfaceDeclarationKind::Conformance
            || source_kind(surface, declaration)? != ModuleSourceKind::Root
        {
            continue;
        }
        roots
            .entry(conformance_key(surface, declaration)?)
            .or_default()
            .push(SurfaceDeclarationId::from_index(index));
    }

    let mut definitions = BTreeMap::<SurfaceDeclarationId, SurfaceDeclarationId>::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != SurfaceDeclarationKind::Conformance
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        let definition = SurfaceDeclarationId::from_index(index);
        let candidates = roots
            .get(&conformance_key(surface, declaration)?)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|contract| {
                reciprocal_include(
                    surface,
                    surface.declarations()[contract.index()].source(),
                    declaration.source(),
                )
            })
            .collect::<Vec<_>>();
        let contract = match candidates.as_slice() {
            [] => {
                return Err(DeclarationContractError::UncontractedConformance(
                    declaration.node(),
                ));
            }
            [contract] => *contract,
            [contract, conflicting, ..] => {
                return Err(DeclarationContractError::AmbiguousConformanceContract {
                    contract: surface.declarations()[contract.index()].node(),
                    conflicting: surface.declarations()[conflicting.index()].node(),
                });
            }
        };
        if definitions.insert(contract, definition).is_some() {
            return Err(DeclarationContractError::DuplicateConformanceDefinition {
                contract: surface.declarations()[contract.index()].node(),
                definition: declaration.node(),
            });
        }
        if let Some(method) = direct_child_of_kind(
            surface,
            surface.declarations()[contract.index()],
            NodeKind::ConformMethod,
        )? {
            return Err(DeclarationContractError::InvalidConformanceSplit(method));
        }
        if let Some(binding) =
            direct_child_of_kind(surface, declaration, NodeKind::AssociatedTypeBinding)?
        {
            return Err(DeclarationContractError::InvalidConformanceSplit(binding));
        }
        representatives[definition.index()] = contract;
    }
    Ok(())
}

fn direct_child_of_kind(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
    expected: NodeKind,
) -> Result<Option<NodeId>, DeclarationContractError> {
    let tree = surface
        .sources()
        .get(declaration.source().index())
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))?
        .syntax();
    for child in tree.children(declaration.node()) {
        let SyntaxElement::Node(child) = child else {
            continue;
        };
        if tree
            .node(*child)
            .is_some_and(|node| node.kind() == expected)
        {
            return Ok(Some(*child));
        }
    }
    Ok(None)
}

fn conformance_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<ConformanceKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok(ConformanceKey {
        module: source.module().clone(),
        header: fingerprint(surface, declaration)?,
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
) -> Result<BodyCandidates, DeclarationContractError> {
    let mut exact_bodies: BTreeMap<CallableKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    let mut loose_bodies: BTreeMap<LooseCallableKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if !is_separable_callable(declaration)
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        if !has_body(surface, declaration)? {
            return Err(DeclarationContractError::InvalidBodyOmission(
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
) -> Result<JoinedBodies, DeclarationContractError> {
    let mut used_bodies = BTreeSet::new();
    let mut container_targets: BTreeMap<SurfaceDeclarationId, BTreeSet<SurfaceDeclarationId>> =
        BTreeMap::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if !is_separable_callable(contract) || has_body(surface, contract)? {
            continue;
        }
        if !is_eligible_contract(surface, contract)? {
            return Err(DeclarationContractError::InvalidBodyOmission(
                contract.node(),
            ));
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let (exact, loose) = callable_keys(surface, contract)?;
        let exact = candidates
            .exact
            .get(&exact)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|body| {
                reciprocal_include(
                    surface,
                    contract.source(),
                    surface.declarations()[body.index()].source(),
                )
            })
            .collect::<Vec<_>>();
        match exact.as_slice() {
            [] => {
                if let Some(body) = candidates.loose.get(&loose).and_then(|bodies| {
                    bodies.iter().find(|body| {
                        reciprocal_include(
                            surface,
                            contract.source(),
                            surface.declarations()[body.index()].source(),
                        )
                    })
                }) {
                    return Err(DeclarationContractError::MismatchedBody {
                        contract: contract.node(),
                        body: surface.declarations()[body.index()].node(),
                    });
                }
                return Err(DeclarationContractError::MissingBody(contract.node()));
            }
            [body] => {
                if !used_bodies.insert(*body) {
                    return Err(DeclarationContractError::DuplicateBody {
                        contract: contract.node(),
                        body: surface.declarations()[body.index()].node(),
                    });
                }
                representatives[body.index()] = contract_id;
                join_nested_contract_declarations(surface, contract_id, *body, representatives)?;
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
                return Err(DeclarationContractError::DuplicateBody {
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

fn join_nested_contract_declarations(
    surface: &DeclarationSurface<'_>,
    contract: SurfaceDeclarationId,
    body: SurfaceDeclarationId,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let contract_nested = nested_contract_declarations(surface, contract);
    let body_nested = nested_contract_declarations(surface, body);
    if contract_nested.len() != body_nested.len() {
        return Err(DeclarationContractError::InconsistentSurface(
            surface.declarations()[body.index()].node(),
        ));
    }
    for (contract_nested, body_nested) in contract_nested.into_iter().zip(body_nested) {
        let contract_declaration = surface.declarations()[contract_nested.index()];
        let body_declaration = surface.declarations()[body_nested.index()];
        if contract_declaration.kind() != body_declaration.kind()
            || fingerprint(surface, contract_declaration)?
                != fingerprint(surface, body_declaration)?
        {
            return Err(DeclarationContractError::InconsistentSurface(
                body_declaration.node(),
            ));
        }
        representatives[body_nested.index()] = contract_nested;
        join_nested_contract_declarations(surface, contract_nested, body_nested, representatives)?;
    }
    Ok(())
}

fn nested_contract_declarations(
    surface: &DeclarationSurface<'_>,
    owner: SurfaceDeclarationId,
) -> Vec<SurfaceDeclarationId> {
    surface
        .declarations()
        .iter()
        .enumerate()
        .filter(|(_, declaration)| declaration.owner() == Some(owner))
        .map(|(index, _)| SurfaceDeclarationId::from_index(index))
        .collect()
}

pub(super) fn reciprocal_include(
    surface: &DeclarationSurface<'_>,
    contract: crate::SurfaceSourceId,
    definition: crate::SurfaceSourceId,
) -> bool {
    surface
        .includes()
        .iter()
        .any(|see| see.source() == contract && see.target() == definition)
        && surface
            .includes()
            .iter()
            .any(|see| see.source() == definition && see.target() == contract)
}

fn join_implementation_containers(
    surface: &DeclarationSurface<'_>,
    used_bodies: &BTreeSet<SurfaceDeclarationId>,
    container_targets: BTreeMap<SurfaceDeclarationId, BTreeSet<SurfaceDeclarationId>>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    for (implementation_owner, targets) in container_targets {
        if targets.len() != 1 {
            continue;
        }
        representatives[implementation_owner.index()] =
            *targets
                .first()
                .ok_or(DeclarationContractError::InconsistentSurface(
                    surface.declarations()[implementation_owner.index()].node(),
                ))?;
    }
    validate_implementation_conformances(surface, representatives)?;
    validate_implementation_interface_defaults(surface, used_bodies, representatives)?;
    Ok(())
}

fn validate_implementation_conformances(
    surface: &DeclarationSurface<'_>,
    representatives: &[SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != SurfaceDeclarationKind::Conformance
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        let id = SurfaceDeclarationId::from_index(index);
        if representatives[id.index()] == id {
            return Err(DeclarationContractError::UncontractedConformance(
                declaration.node(),
            ));
        }
    }
    Ok(())
}

fn validate_implementation_interface_defaults(
    surface: &DeclarationSurface<'_>,
    used_bodies: &BTreeSet<SurfaceDeclarationId>,
    representatives: &[SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != SurfaceDeclarationKind::Interface
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        let id = SurfaceDeclarationId::from_index(index);
        let defaults = surface
            .declarations()
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, child)| {
                child.owner() == Some(id) && child.kind() == SurfaceDeclarationKind::InterfaceMethod
            })
            .map(|(child, declaration)| (SurfaceDeclarationId::from_index(child), declaration))
            .collect::<Vec<_>>();
        if defaults.is_empty() {
            continue;
        }
        if representatives[id.index()] == id {
            return Err(DeclarationContractError::UncontractedInterfaceDefault(
                declaration.node(),
            ));
        }
        if let Some((_, child)) = defaults
            .into_iter()
            .find(|(child, _)| !used_bodies.contains(child))
        {
            return Err(DeclarationContractError::UncontractedInterfaceDefault(
                child.node(),
            ));
        }
    }
    Ok(())
}

fn callable_keys(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<(CallableKey, LooseCallableKey), DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let header = fingerprint(surface, declaration)?;
    let owner = declaration
        .owner()
        .map(|owner| {
            let owner = surface.declarations().get(owner.index()).copied().ok_or(
                DeclarationContractError::InconsistentSurface(declaration.node()),
            )?;
            fingerprint(surface, owner)
        })
        .transpose()?;
    let label = callable_label(declaration.kind(), &header).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
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

pub(super) fn fingerprint(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<HeaderFingerprint, DeclarationContractError> {
    let surface_source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let tree = surface_source.syntax();
    let source = surface.source_map().get(tree.source()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    tree.node(declaration.node())
        .ok_or(DeclarationContractError::InconsistentSurface(
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
                    .ok_or(DeclarationContractError::InconsistentSurface(
                        declaration.node(),
                    ))?
                    .kind();
                if kind == NodeKind::Visibility
                    || kind == NodeKind::Block
                    || declaration.kind() == SurfaceDeclarationKind::Constant
                        && kind == NodeKind::Expression
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
                    DeclarationContractError::InconsistentSurface(declaration.node()),
                )?;
                if declaration.kind() == SurfaceDeclarationKind::Constant
                    && depth == 0
                    && spelling == "="
                {
                    continue;
                }
                tokens.push(Box::<str>::from(spelling));
            }
            SyntaxElement::Missing(_) => {
                return Err(DeclarationContractError::InconsistentSurface(
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
        SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::InherentMethod
        | SurfaceDeclarationKind::ConformanceMethod => tokens
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
        SurfaceDeclarationKind::Equality
        | SurfaceDeclarationKind::Ordering
        | SurfaceDeclarationKind::Index
        | SurfaceDeclarationKind::Expansion => Some(CallableLabel::Operator),
        _ => None,
    }
}

pub(super) fn source_kind(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<ModuleSourceKind, DeclarationContractError> {
    surface
        .sources()
        .get(declaration.source().index())
        .map(crate::SurfaceSource::kind)
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))
}

fn has_body(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<bool, DeclarationContractError> {
    let tree = surface
        .sources()
        .get(declaration.source().index())
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))?
        .syntax();
    let mut pending = vec![declaration.node()];
    while let Some(node) = pending.pop() {
        for child in tree.children(node) {
            if let SyntaxElement::Node(child) = child {
                let kind = tree
                    .node(*child)
                    .ok_or(DeclarationContractError::InconsistentSurface(
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

fn is_separable_callable(declaration: SurfaceDeclaration) -> bool {
    matches!(
        declaration.kind(),
        SurfaceDeclarationKind::Function
            | SurfaceDeclarationKind::InherentMethod
            | SurfaceDeclarationKind::ConstructionFunction
            | SurfaceDeclarationKind::Literal
            | SurfaceDeclarationKind::Coercion
            | SurfaceDeclarationKind::Equality
            | SurfaceDeclarationKind::Ordering
            | SurfaceDeclarationKind::Index
            | SurfaceDeclarationKind::Expansion
            | SurfaceDeclarationKind::ConformanceMethod
    ) || declaration.kind() == SurfaceDeclarationKind::InterfaceMethod
        && declaration.is_interface_default()
}

pub(super) fn is_eligible_contract(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<bool, DeclarationContractError> {
    if source_kind(surface, declaration)? != ModuleSourceKind::Root {
        return Ok(false);
    }
    if declaration.visibility().is_some() {
        return Ok(true);
    }
    Ok(false)
}

const fn is_member_declaration(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::ConstantDeclaration
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

#[cfg(test)]
mod tests;
