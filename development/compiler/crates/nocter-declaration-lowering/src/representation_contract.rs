use std::collections::{BTreeMap, BTreeSet};

use nocter_syntax::{SyntaxElement, TokenKind};

use crate::contract::{
    HeaderFingerprint, definition_candidates, fingerprint, has_reciprocal_source_visibility,
    is_access_source, source_kind,
};
use crate::{
    DeclarationContractError, DeclarationSurface, ModuleIdentity, ModuleSourceKind,
    StructuralGrantIssue, SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
};

mod structural;
pub(super) use structural::collect_structural_grants;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NominalKey {
    module: ModuleIdentity,
    kind: SurfaceDeclarationKind,
    header: HeaderFingerprint,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LooseNominalKey {
    module: ModuleIdentity,
    name: Box<str>,
}

#[derive(Default)]
struct NominalDefinitions {
    exact: BTreeMap<NominalKey, Vec<SurfaceDeclarationId>>,
    loose: BTreeMap<LooseNominalKey, Vec<SurfaceDeclarationId>>,
}
impl NominalDefinitions {
    fn collect(surface: &DeclarationSurface<'_>) -> Result<Self, DeclarationContractError> {
        let mut definitions = Self::default();
        for (index, declaration) in surface.declarations().iter().copied().enumerate() {
            if !is_nominal(declaration.kind())
                || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
            {
                continue;
            }
            let id = SurfaceDeclarationId::from_index(index);
            definitions
                .exact
                .entry(nominal_key(surface, declaration)?)
                .or_default()
                .push(id);
            definitions
                .loose
                .entry(loose_nominal_key(surface, declaration)?)
                .or_default()
                .push(id);
        }
        Ok(definitions)
    }
}

/// Joins public opaque nominal contracts to the private representation occurrence that completes
/// their fields or variants. Callable-body matching remains a separate responsibility.
pub(super) fn join_nominal_contracts(
    surface: &DeclarationSurface<'_>,
    structural_grants: &BTreeMap<SurfaceDeclarationId, SurfaceDeclarationId>,
    representatives: &mut [SurfaceDeclarationId],
    representations: &mut [Option<SurfaceDeclarationId>],
) -> Result<(), DeclarationContractError> {
    let definitions = NominalDefinitions::collect(surface)?;

    let mut used = BTreeSet::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if !is_nominal(contract.kind()) {
            continue;
        }
        let requires = requires_representation(surface, contract)?;
        let closed_enum = !requires
            && contract.kind() == SurfaceDeclarationKind::Enum
            && source_kind(surface, contract)? == ModuleSourceKind::Root
            && is_access_source(surface, contract)?;
        if !requires && !closed_enum {
            continue;
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let loose_candidates = definitions
            .loose
            .get(&loose_nominal_key(surface, contract)?)
            .map_or(&[][..], Vec::as_slice);
        let reciprocal_definition = || visible_definition(surface, contract, loose_candidates);
        if closed_enum {
            if let Some(definition) = reciprocal_definition() {
                return Err(DeclarationContractError::DuplicateRepresentation {
                    contract: contract.node(),
                    definition: definition.node(),
                });
            }
            continue;
        }
        if let Some(grant) = structural_grants.get(&contract_id) {
            if let Some(definition) = reciprocal_definition() {
                return Err(DeclarationContractError::InvalidStructuralGrant {
                    primary: definition.node(),
                    related: Some(surface.declarations()[grant.index()].node()),
                    issue: StructuralGrantIssue::DuplicateRepresentation,
                });
            }
            continue;
        }
        let candidates = definition_candidates(
            surface,
            contract,
            definitions
                .exact
                .get(&nominal_key(surface, contract)?)
                .map_or(&[], Vec::as_slice),
            loose_candidates,
        )?;
        match candidates.as_slice() {
            [] => {
                if let Some(definition) = reciprocal_definition() {
                    return Err(DeclarationContractError::MismatchedRepresentation {
                        contract: contract.node(),
                        definition: definition.node(),
                    });
                }
                return Err(DeclarationContractError::MissingRepresentation(
                    contract.node(),
                ));
            }
            [definition] => {
                if is_access_source(surface, contract)?
                    && fingerprint(surface, contract)?
                        != fingerprint(surface, surface.declarations()[definition.index()])?
                {
                    return Err(DeclarationContractError::MismatchedRepresentation {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                if !used.insert(*definition) {
                    return Err(DeclarationContractError::RepresentationCompletedAgain {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                if is_access_source(surface, contract)? {
                    join_visible_fields(surface, contract_id, *definition, representatives)?;
                }
                representatives[definition.index()] = contract_id;
                representations[contract_id.index()] = Some(*definition);
            }
            [_, second, ..] => {
                return Err(DeclarationContractError::DuplicateRepresentation {
                    contract: contract.node(),
                    definition: surface.declarations()[second.index()].node(),
                });
            }
        }
    }
    Ok(())
}

fn visible_definition(
    surface: &DeclarationSurface<'_>,
    contract: SurfaceDeclaration,
    candidates: &[SurfaceDeclarationId],
) -> Option<SurfaceDeclaration> {
    candidates
        .iter()
        .map(|id| surface.declarations()[id.index()])
        .find(|definition| {
            has_reciprocal_source_visibility(surface, contract.source(), definition.source())
        })
}

fn requires_representation(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<bool, DeclarationContractError> {
    Ok(is_bodyless_nominal(surface, declaration)
        || declaration.kind() == SurfaceDeclarationKind::Struct
            && source_kind(surface, declaration)? == ModuleSourceKind::Root
            && is_access_source(surface, declaration)?)
}

fn join_visible_fields(
    surface: &DeclarationSurface<'_>,
    contract: SurfaceDeclarationId,
    definition: SurfaceDeclarationId,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let mismatch = || DeclarationContractError::MismatchedRepresentation {
        contract: surface.declarations()[contract.index()].node(),
        definition: surface.declarations()[definition.index()].node(),
    };
    let mut remaining = surface.record_fields(definition);
    for (contract_id, field) in surface.record_fields(contract) {
        let name = field_name(surface, field)?;
        let mut selected = None;
        for (id, candidate) in remaining.by_ref() {
            if field_name(surface, candidate)? == name {
                selected = Some((id, candidate));
                break;
            }
        }
        let Some((definition_id, candidate)) = selected else {
            return Err(mismatch());
        };
        if fingerprint(surface, field)? != fingerprint(surface, candidate)? {
            return Err(mismatch());
        }
        representatives[definition_id.index()] = contract_id;
    }
    Ok(())
}
fn field_name<'a>(
    surface: &'a DeclarationSurface<'_>,
    field: SurfaceDeclaration,
) -> Result<&'a str, DeclarationContractError> {
    field
        .name()
        .and_then(|name| {
            surface
                .source_map()
                .get(name.source())?
                .text_at(name.range())
        })
        .ok_or(DeclarationContractError::InconsistentSurface(field.node()))
}

fn nominal_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<NominalKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok(NominalKey {
        module: source.module().clone(),
        kind: declaration.kind(),
        header: fingerprint(surface, declaration)?,
    })
}

fn loose_nominal_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<LooseNominalKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let name = declaration
        .name()
        .and_then(|token| {
            surface
                .source_map()
                .get(token.source())?
                .text_at(token.range())
        })
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))?;
    Ok(LooseNominalKey {
        module: source.module().clone(),
        name: name.into(),
    })
}

fn is_bodyless_nominal(surface: &DeclarationSurface<'_>, declaration: SurfaceDeclaration) -> bool {
    let tree = surface.sources()[declaration.source().index()].syntax();
    !tree.children(declaration.node()).iter().any(|element| {
        matches!(
            element,
            SyntaxElement::Token(token)
                if token.kind()
                    == TokenKind::Punctuation(nocter_syntax::Punctuation::LeftBrace)
        )
    })
}

const fn is_nominal(kind: SurfaceDeclarationKind) -> bool {
    matches!(
        kind,
        SurfaceDeclarationKind::Struct | SurfaceDeclarationKind::Enum
    )
}
