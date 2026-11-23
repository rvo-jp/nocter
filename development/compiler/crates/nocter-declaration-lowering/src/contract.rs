use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_syntax::{
    BoundSyntax, NodeId, NodeKind, Punctuation, SyntaxStructure, TokenKind, declaration_body,
    direct_node, project_declaration_header, project_syntax_structure,
};

use crate::{
    DeclarationSurface, ModuleIdentity, ModuleSourceKind, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

mod compile_time_value;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HeaderFingerprint(pub(super) SyntaxStructure);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CallableLabel {
    Named(Box<str>),
    LiteralSequence,
    LiteralString,
    Coercion {
        receiver: SyntaxStructure,
        target: SyntaxStructure,
    },
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

/// The canonical source-declaration representative selected for every authored surface.
///
/// A public bodyless callable and its private implementation body share the public contract's
/// representative. All other declarations initially represent themselves. This structural join
/// does not resolve the two headers or publish normalized semantic types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationContracts {
    representatives: Box<[SurfaceDeclarationId]>,
    representations: Box<[Option<SurfaceDeclarationId>]>,
    structural_grants: BTreeMap<SurfaceDeclarationId, SurfaceDeclarationId>,
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

    pub(super) fn structural_grants(
        &self,
    ) -> impl Iterator<Item = (SurfaceDeclarationId, SurfaceDeclarationId)> + '_ {
        self.structural_grants
            .iter()
            .map(|(&record, &grant)| (record, grant))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationContractError {
    MissingCompileTimeInitializer(NodeId),
    MismatchedCompileTimeInitializer {
        contract: NodeId,
        definition: NodeId,
    },
    DuplicateCompileTimeInitializer {
        contract: NodeId,
        definition: NodeId,
    },
    InvalidCompileTimeOmission(NodeId),
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
    InvalidStructuralGrant {
        primary: NodeId,
        related: Option<NodeId>,
        issue: StructuralGrantIssue,
    },
    InterfaceImplementationOutsideRoot(NodeId),
    UncontractedInterfaceDefault(NodeId),
    InconsistentSurface(NodeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralGrantIssue {
    RootSchema,
    FieldList,
    DuplicateGrant,
    DuplicateRepresentation,
}

impl StructuralGrantIssue {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::RootSchema => {
                "a structural literal grant requires a braced struct contract in this index.nct"
            }
            Self::FieldList => {
                "a structural literal grant must list every contract field once in declaration order"
            }
            Self::DuplicateGrant => "a struct permits one structural literal grant",
            Self::DuplicateRepresentation => {
                "the structural literal grant already closes this struct's representation"
            }
        }
    }
}

impl fmt::Display for DeclarationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCompileTimeInitializer(contract) => write!(
                formatter,
                "compile-time value contract {contract:?} has no initializer definition"
            ),
            Self::MismatchedCompileTimeInitializer {
                contract,
                definition,
            } => write!(
                formatter,
                "compile-time initializer {definition:?} does not match contract {contract:?}"
            ),
            Self::DuplicateCompileTimeInitializer {
                contract,
                definition,
            } => write!(
                formatter,
                "compile-time value contract {contract:?} has duplicate initializer {definition:?}"
            ),
            Self::InvalidCompileTimeOmission(node) => write!(
                formatter,
                "compile-time value {node:?} omits its initializer outside an eligible public root contract"
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
            Self::InvalidStructuralGrant { issue, .. } => formatter.write_str(issue.message()),
            Self::InterfaceImplementationOutsideRoot(node) => write!(
                formatter,
                "interface implementation {node:?} must be declared in the module root"
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
/// Header equality uses syntax-owned canonical structure. Only the declaration's own body,
/// visibility and members are excluded; header expression blocks remain exact.
/// No name or type is resolved during this pass.
///
/// # Errors
///
/// Returns [`DeclarationContractError`] for a missing, mismatched, duplicate, or ineligible body.
pub fn analyze_declaration_contracts(
    surface: &DeclarationSurface<'_>,
) -> Result<DeclarationContracts, DeclarationContractError> {
    // Source-role invariants are authored facts. Validate them before any declarations are joined
    // so their meaning cannot depend on which declaration becomes the semantic representative.
    validate_implementation_interface_implementations(surface)?;
    let count = surface.declarations().len();
    let mut representatives: Vec<_> = (0..count).map(SurfaceDeclarationId::from_index).collect();
    let mut representations = vec![None; count];
    let structural_grants = crate::representation_contract::collect_structural_grants(surface)?;
    crate::representation_contract::join_nominal_contracts(
        surface,
        &structural_grants,
        &mut representatives,
        &mut representations,
    )?;
    compile_time_value::join(surface, &mut representatives)?;
    let candidates = collect_body_candidates(surface)?;
    let joined = join_contracts(surface, &candidates, &mut representatives)?;
    join_implementation_containers(
        surface,
        &joined.used,
        joined.container_targets,
        &mut representatives,
    )?;
    join_instance_fragments(surface, &mut representatives)?;

    Ok(DeclarationContracts {
        representatives: representatives.into_boxed_slice(),
        representations: representations.into_boxed_slice(),
        structural_grants,
    })
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InstanceFragmentKey {
    module: ModuleIdentity,
    header: HeaderFingerprint,
}

/// Unifies open `instance` fragments before semantic identities are reserved.
///
/// A root contract may contain only `impl Interface` facts while mutually visible implementation
/// sources contain the satisfying methods. Their identical instance header denotes one semantic
/// container; child declarations remain independently source-backed beneath its representative.
fn join_instance_fragments(
    surface: &DeclarationSurface<'_>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let mut groups = BTreeMap::<InstanceFragmentKey, Vec<SurfaceDeclarationId>>::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if declaration.kind() != SurfaceDeclarationKind::Instance
            || representatives[id.index()] != id
        {
            continue;
        }
        let source = surface.sources().get(declaration.source().index()).ok_or(
            DeclarationContractError::InconsistentSurface(declaration.node()),
        )?;
        groups
            .entry(InstanceFragmentKey {
                module: source.module().clone(),
                header: fingerprint(surface, declaration)?,
            })
            .or_default()
            .push(id);
    }

    for fragments in groups.values_mut() {
        let mut ranked = Vec::with_capacity(fragments.len());
        for fragment in fragments.iter().copied() {
            let declaration = surface.declarations()[fragment.index()];
            let root_rank =
                usize::from(source_kind(surface, declaration)? != ModuleSourceKind::Root);
            ranked.push((root_rank, fragment.index(), fragment));
        }
        ranked.sort_unstable();
        fragments.clear();
        fragments.extend(ranked.into_iter().map(|(_, _, fragment)| fragment));
        let mut component_representatives = Vec::<SurfaceDeclarationId>::new();
        for fragment in fragments.iter().copied() {
            let declaration = surface.declarations()[fragment.index()];
            let representative = component_representatives.iter().copied().find(|candidate| {
                let candidate = surface.declarations()[candidate.index()];
                declaration.source() == candidate.source()
                    || has_reciprocal_source_visibility(
                        surface,
                        declaration.source(),
                        candidate.source(),
                    )
            });
            if let Some(representative) = representative {
                representatives[fragment.index()] = representative;
            } else {
                component_representatives.push(fragment);
            }
        }
    }
    Ok(())
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
        let exact = definition_candidates(
            surface,
            contract,
            candidates.exact.get(&exact).map_or(&[], Vec::as_slice),
            candidates.loose.get(&loose).map_or(&[], Vec::as_slice),
        )?;
        match exact.as_slice() {
            [] => {
                if let Some(body) = candidates.loose.get(&loose).and_then(|bodies| {
                    bodies.iter().find(|body| {
                        has_reciprocal_source_visibility(
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
                if is_access_source(surface, contract)?
                    && fingerprint(surface, contract)?
                        != fingerprint(surface, surface.declarations()[body.index()])?
                {
                    return Err(DeclarationContractError::MismatchedBody {
                        contract: contract.node(),
                        body: surface.declarations()[body.index()].node(),
                    });
                }
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

/// The qualified v2 policy selects by operation identity before comparing complete headers.
/// An exact signature cannot hide another directly paired candidate. Released selection retains
/// its existing matching policy until that product's contract changes separately.
pub(super) fn definition_candidates(
    surface: &DeclarationSurface<'_>,
    contract: SurfaceDeclaration,
    exact: &[SurfaceDeclarationId],
    identity: &[SurfaceDeclarationId],
) -> Result<Vec<SurfaceDeclarationId>, DeclarationContractError> {
    let selected = if is_access_source(surface, contract)? {
        identity
    } else {
        exact
    };
    Ok(selected
        .iter()
        .copied()
        .filter(|definition| {
            has_reciprocal_source_visibility(
                surface,
                contract.source(),
                surface.declarations()[definition.index()].source(),
            )
        })
        .collect())
}

pub(super) fn has_reciprocal_source_visibility(
    surface: &DeclarationSurface<'_>,
    contract: crate::SurfaceSourceId,
    definition: crate::SurfaceSourceId,
) -> bool {
    surface
        .source_visibilities()
        .iter()
        .any(|see| see.source() == contract && see.target() == definition)
        && surface
            .source_visibilities()
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
    validate_implementation_interface_defaults(surface, used_bodies, representatives)?;
    Ok(())
}

fn validate_implementation_interface_implementations(
    surface: &DeclarationSurface<'_>,
) -> Result<(), DeclarationContractError> {
    for declaration in surface.declarations().iter().copied() {
        if declaration.kind() != SurfaceDeclarationKind::InterfaceImplementation
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        return Err(
            DeclarationContractError::InterfaceImplementationOutsideRoot(declaration.node()),
        );
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
    let label = callable_label(surface, declaration).ok_or(
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
    let syntax = bound_syntax(surface, declaration).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let header = if declaration.kind() == SurfaceDeclarationKind::OpaqueType {
        project_syntax_structure(syntax, declaration.node())
    } else {
        project_declaration_header(syntax, declaration.node())
    };
    header
        .map(HeaderFingerprint)
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))
}

pub(super) fn bound_syntax<'a>(
    surface: &'a DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Option<BoundSyntax<'a>> {
    let tree = surface
        .sources()
        .get(declaration.source().index())?
        .syntax();
    BoundSyntax::new(surface.source_map().get(tree.source())?, tree)
}

fn callable_label(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Option<CallableLabel> {
    let syntax = bound_syntax(surface, declaration)?;
    let tree = syntax.tree();
    match declaration.kind() {
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::ConstructionFunction
        | SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::InherentMethod => Some(CallableLabel::Named(
            syntax.text_at(declaration.name()?.range())?.into(),
        )),
        SurfaceDeclarationKind::Literal => {
            let shape = direct_node(tree, declaration.node(), NodeKind::LiteralShape)?;
            if nocter_syntax::direct_token_iter(tree, shape)
                .any(|token| token.kind() == TokenKind::Punctuation(Punctuation::LeftBracket))
            {
                Some(CallableLabel::LiteralSequence)
            } else {
                Some(CallableLabel::LiteralString)
            }
        }
        SurfaceDeclarationKind::Coercion => Some(CallableLabel::Coercion {
            receiver: project_syntax_structure(
                syntax,
                direct_node(tree, declaration.node(), NodeKind::Receiver)?,
            )?,
            target: project_syntax_structure(
                syntax,
                direct_node(tree, declaration.node(), NodeKind::Type)?,
            )?,
        }),
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
    let syntax = bound_syntax(surface, declaration).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok(declaration_body(syntax.tree(), declaration.node()).is_some())
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
    Ok(declaration.visibility().is_some() || is_access_source(surface, declaration)?)
}

pub(super) fn is_access_source(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<bool, DeclarationContractError> {
    let syntax = bound_syntax(surface, declaration).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    match syntax.tree().root().kind() {
        NodeKind::V2CoreUnit => Ok(true),
        NodeKind::SourceFile => Ok(false),
        _ => Err(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        )),
    }
}

#[cfg(test)]
mod tests;
