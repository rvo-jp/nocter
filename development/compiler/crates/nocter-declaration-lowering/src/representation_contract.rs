use std::collections::{BTreeMap, BTreeSet};

use nocter_syntax::{SyntaxElement, TokenKind};

use crate::contract::{
    HeaderFingerprint, definition_candidates, fingerprint, has_reciprocal_source_visibility,
    is_access_source, source_kind,
};
use crate::{
    DeclarationContractError, DeclarationSurface, ModuleIdentity, ModuleSourceKind,
    SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
};

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

/// Joins public opaque nominal contracts to the private representation occurrence that completes
/// their fields or variants. Callable-body matching remains a separate responsibility.
pub(super) fn join_nominal_contracts(
    surface: &DeclarationSurface<'_>,
    representatives: &mut [SurfaceDeclarationId],
    representations: &mut [Option<SurfaceDeclarationId>],
) -> Result<(), DeclarationContractError> {
    let mut exact: BTreeMap<NominalKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    let mut loose: BTreeMap<LooseNominalKey, Vec<SurfaceDeclarationId>> = BTreeMap::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if !is_nominal(declaration.kind())
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        let id = SurfaceDeclarationId::from_index(index);
        exact
            .entry(nominal_key(surface, declaration)?)
            .or_default()
            .push(id);
        loose
            .entry(loose_nominal_key(surface, declaration)?)
            .or_default()
            .push(id);
    }

    let mut used = BTreeSet::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if !is_nominal(contract.kind()) || !requires_representation(surface, contract)? {
            continue;
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let candidates = definition_candidates(
            surface,
            contract,
            exact
                .get(&nominal_key(surface, contract)?)
                .map_or(&[], Vec::as_slice),
            loose
                .get(&loose_nominal_key(surface, contract)?)
                .map_or(&[], Vec::as_slice),
        )?;
        match candidates.as_slice() {
            [] => {
                let mismatch = loose
                    .get(&loose_nominal_key(surface, contract)?)
                    .into_iter()
                    .flatten()
                    .copied()
                    .find(|definition| {
                        has_reciprocal_source_visibility(
                            surface,
                            contract.source(),
                            surface.declarations()[definition.index()].source(),
                        )
                    });
                if let Some(definition) = mismatch {
                    return Err(DeclarationContractError::MismatchedRepresentation {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
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
    let fields = |owner| {
        surface
            .declarations()
            .iter()
            .copied()
            .enumerate()
            .filter(move |(_, field)| {
                field.owner() == Some(owner) && field.kind() == SurfaceDeclarationKind::Field
            })
            .map(|(index, field)| (SurfaceDeclarationId::from_index(index), field))
    };
    let mismatch = || DeclarationContractError::MismatchedRepresentation {
        contract: surface.declarations()[contract.index()].node(),
        definition: surface.declarations()[definition.index()].node(),
    };
    let mut remaining = fields(definition);
    for (contract_id, field) in fields(contract) {
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
