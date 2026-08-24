use std::collections::{BTreeMap, BTreeSet};

use nocter_syntax::{NodeKind, SyntaxElement};

use crate::{
    DeclarationSurface, ModuleIdentity, ModuleSourceKind, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

use super::{
    DeclarationContractError, HeaderFingerprint, fingerprint, has_reciprocal_source_visibility,
    is_eligible_contract, source_kind,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConstantKey {
    module: ModuleIdentity,
    header: HeaderFingerprint,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LooseConstantKey {
    module: ModuleIdentity,
    name: Box<str>,
}

/// Joins one public root constant contract to one private initializer definition.
pub(super) fn join(
    surface: &DeclarationSurface<'_>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let mut exact = BTreeMap::<ConstantKey, Vec<SurfaceDeclarationId>>::new();
    let mut loose = BTreeMap::<LooseConstantKey, Vec<SurfaceDeclarationId>>::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != SurfaceDeclarationKind::Constant
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        if !has_initializer(surface, declaration)? {
            return Err(DeclarationContractError::InvalidConstantOmission(
                declaration.node(),
            ));
        }
        let id = SurfaceDeclarationId::from_index(index);
        exact
            .entry(constant_key(surface, declaration)?)
            .or_default()
            .push(id);
        loose
            .entry(loose_constant_key(surface, declaration)?)
            .or_default()
            .push(id);
    }

    let mut used = BTreeSet::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if contract.kind() != SurfaceDeclarationKind::Constant
            || has_initializer(surface, contract)?
        {
            continue;
        }
        if !is_eligible_contract(surface, contract)? {
            return Err(DeclarationContractError::InvalidConstantOmission(
                contract.node(),
            ));
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let candidates = exact
            .get(&constant_key(surface, contract)?)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|definition| {
                has_reciprocal_source_visibility(
                    surface,
                    contract.source(),
                    surface.declarations()[definition.index()].source(),
                )
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {
                if let Some(definition) = loose
                    .get(&loose_constant_key(surface, contract)?)
                    .and_then(|definitions| {
                        definitions.iter().find(|definition| {
                            has_reciprocal_source_visibility(
                                surface,
                                contract.source(),
                                surface.declarations()[definition.index()].source(),
                            )
                        })
                    })
                {
                    return Err(DeclarationContractError::MismatchedConstantInitializer {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                return Err(DeclarationContractError::MissingConstantInitializer(
                    contract.node(),
                ));
            }
            [definition] => {
                if !used.insert(*definition) {
                    return Err(DeclarationContractError::DuplicateConstantInitializer {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                representatives[definition.index()] = contract_id;
            }
            [_, duplicate, ..] => {
                return Err(DeclarationContractError::DuplicateConstantInitializer {
                    contract: contract.node(),
                    definition: surface.declarations()[duplicate.index()].node(),
                });
            }
        }
    }
    Ok(())
}

fn constant_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<ConstantKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok(ConstantKey {
        module: source.module().clone(),
        header: fingerprint(surface, declaration)?,
    })
}

fn loose_constant_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<LooseConstantKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let name = declaration
        .name()
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))?;
    let text = surface
        .source_map()
        .get(name.source())
        .and_then(|source| source.text_at(name.range()))
        .ok_or(DeclarationContractError::InconsistentSurface(
            declaration.node(),
        ))?;
    Ok(LooseConstantKey {
        module: source.module().clone(),
        name: text.into(),
    })
}

fn has_initializer(
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
    Ok(tree.children(declaration.node()).iter().any(|element| {
        matches!(element, SyntaxElement::Node(node) if tree.node(*node).is_some_and(|node| node.kind() == NodeKind::Expression))
    }))
}
