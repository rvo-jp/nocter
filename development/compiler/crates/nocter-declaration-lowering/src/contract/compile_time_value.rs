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
struct CompileTimeValueKey {
    module: ModuleIdentity,
    kind: SurfaceDeclarationKind,
    header: HeaderFingerprint,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LooseCompileTimeValueKey {
    module: ModuleIdentity,
    kind: SurfaceDeclarationKind,
    name: Box<str>,
}

/// Joins one public root const or static contract to one private initializer definition.
pub(super) fn join(
    surface: &DeclarationSurface<'_>,
    representatives: &mut [SurfaceDeclarationId],
) -> Result<(), DeclarationContractError> {
    let mut exact = BTreeMap::<CompileTimeValueKey, Vec<SurfaceDeclarationId>>::new();
    let mut loose = BTreeMap::<LooseCompileTimeValueKey, Vec<SurfaceDeclarationId>>::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if !is_compile_time_value(declaration.kind())
            || source_kind(surface, declaration)? != ModuleSourceKind::Implementation
        {
            continue;
        }
        if !has_initializer(surface, declaration)? {
            return Err(DeclarationContractError::InvalidCompileTimeOmission(
                declaration.node(),
            ));
        }
        let id = SurfaceDeclarationId::from_index(index);
        exact
            .entry(compile_time_value_key(surface, declaration)?)
            .or_default()
            .push(id);
        loose
            .entry(loose_compile_time_value_key(surface, declaration)?)
            .or_default()
            .push(id);
    }

    let mut used = BTreeSet::new();
    for (index, contract) in surface.declarations().iter().copied().enumerate() {
        if !is_compile_time_value(contract.kind()) || has_initializer(surface, contract)? {
            continue;
        }
        if !is_eligible_contract(surface, contract)? {
            return Err(DeclarationContractError::InvalidCompileTimeOmission(
                contract.node(),
            ));
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let candidates = exact
            .get(&compile_time_value_key(surface, contract)?)
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
                    .get(&loose_compile_time_value_key(surface, contract)?)
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
                    return Err(DeclarationContractError::MismatchedCompileTimeInitializer {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                return Err(DeclarationContractError::MissingCompileTimeInitializer(
                    contract.node(),
                ));
            }
            [definition] => {
                if !used.insert(*definition) {
                    return Err(DeclarationContractError::DuplicateCompileTimeInitializer {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
                }
                representatives[definition.index()] = contract_id;
            }
            [_, duplicate, ..] => {
                return Err(DeclarationContractError::DuplicateCompileTimeInitializer {
                    contract: contract.node(),
                    definition: surface.declarations()[duplicate.index()].node(),
                });
            }
        }
    }
    Ok(())
}

fn compile_time_value_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<CompileTimeValueKey, DeclarationContractError> {
    let source = surface.sources().get(declaration.source().index()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    Ok(CompileTimeValueKey {
        module: source.module().clone(),
        kind: declaration.kind(),
        header: fingerprint(surface, declaration)?,
    })
}

fn loose_compile_time_value_key(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<LooseCompileTimeValueKey, DeclarationContractError> {
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
    Ok(LooseCompileTimeValueKey {
        module: source.module().clone(),
        kind: declaration.kind(),
        name: text.into(),
    })
}

const fn is_compile_time_value(kind: SurfaceDeclarationKind) -> bool {
    matches!(
        kind,
        SurfaceDeclarationKind::Constant | SurfaceDeclarationKind::Static
    )
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
