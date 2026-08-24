use std::collections::{BTreeMap, BTreeSet};

use nocter_syntax::{NodeKind, SyntaxElement, TokenKind};

use crate::contract::{HeaderFingerprint, has_reciprocal_source_visibility, source_kind};
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
        if !is_nominal(contract.kind()) || !is_bodyless_nominal(surface, contract) {
            continue;
        }
        let contract_id = SurfaceDeclarationId::from_index(index);
        let candidates = exact
            .get(&nominal_key(surface, contract)?)
            .into_iter()
            .flatten()
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
                if !used.insert(*definition) {
                    return Err(DeclarationContractError::RepresentationCompletedAgain {
                        contract: contract.node(),
                        definition: surface.declarations()[definition.index()].node(),
                    });
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
        header: nominal_fingerprint(surface, declaration)?,
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

fn nominal_fingerprint(
    surface: &DeclarationSurface<'_>,
    declaration: SurfaceDeclaration,
) -> Result<HeaderFingerprint, DeclarationContractError> {
    let source = &surface.sources()[declaration.source().index()];
    let tree = source.syntax();
    let text = surface.source_map().get(tree.source()).ok_or(
        DeclarationContractError::InconsistentSurface(declaration.node()),
    )?;
    let mut tokens = Vec::new();
    let mut pending: Vec<_> = tree
        .children(declaration.node())
        .iter()
        .rev()
        .copied()
        .collect();
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => {
                if tree.node(node).map(nocter_syntax::SyntaxNode::kind)
                    == Some(NodeKind::Visibility)
                {
                    continue;
                }
                pending.extend(tree.children(node).iter().rev().copied());
            }
            SyntaxElement::Token(token) => {
                if token.kind() == TokenKind::Punctuation(nocter_syntax::Punctuation::LeftBrace) {
                    break;
                }
                if matches!(token.kind(), TokenKind::Newline | TokenKind::Eof) {
                    continue;
                }
                tokens.push(
                    text.text_at(token.range())
                        .ok_or(DeclarationContractError::InconsistentSurface(
                            declaration.node(),
                        ))?
                        .into(),
                );
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
