use std::collections::HashMap;

use nocter_declarations::{
    CallableKind, CallableProvenance, CallableProvenanceContract, ProvenanceAnnotation,
    ProvenanceOrigin,
};
use nocter_model::{BodyId, ParameterId, TypeId};
use nocter_source_index::{SemanticEntity, SourceRole, SyntaxOrigin};
use nocter_syntax::{NodeKind, SyntaxElement, SyntaxToken, TokenKind};

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::allocation::surface_node;
use super::super::{
    DefinitionRule, DefinitionViolation, HeaderDefinitionError, projection, syntax,
};

pub(super) fn contract(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    kind: CallableKind,
    receiver: Option<ParameterId>,
    parameters: &[ParameterId],
    result: TypeId,
    body: Option<BodyId>,
) -> Result<(CallableProvenanceContract, ProvenanceAnnotation), HeaderDefinitionError> {
    if let Some((origins, includes_static)) = explicit(types, declaration, receiver, parameters)? {
        return Ok((
            CallableProvenanceContract::declared(origins),
            ProvenanceAnnotation::Explicit { includes_static },
        ));
    }
    if !types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .types()
        .may_carry_storage(result)
    {
        return Ok((
            CallableProvenanceContract::declared(CallableProvenance::empty()),
            ProvenanceAnnotation::Elided,
        ));
    }
    if kind == CallableKind::Coercion {
        let receiver = receiver.ok_or(HeaderDefinitionError::InvalidProvenance(declaration))?;
        let _ = receiver;
        return declared([ProvenanceOrigin::Receiver], declaration)
            .map(|contract| (contract, ProvenanceAnnotation::Elided));
    }
    if body.is_some() {
        return Ok((
            CallableProvenanceContract::inferred(),
            ProvenanceAnnotation::Elided,
        ));
    }
    let store = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .types();
    let declarations = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations();
    let mut candidates = Vec::new();
    if let Some(receiver) = receiver {
        let parameter = declarations
            .parameter(receiver)
            .ok_or(HeaderDefinitionError::InvalidProvenance(declaration))?;
        if store.may_carry_storage(parameter.ty()) {
            candidates.push(ProvenanceOrigin::Receiver);
        }
    }
    for parameter in parameters {
        let definition = declarations
            .parameter(*parameter)
            .ok_or(HeaderDefinitionError::InvalidProvenance(declaration))?;
        if store.may_carry_storage(definition.ty()) {
            candidates.push(ProvenanceOrigin::Parameter(*parameter));
        }
    }
    match candidates.as_slice() {
        [] | [_] => declared(candidates, declaration)
            .map(|contract| (contract, ProvenanceAnnotation::Elided)),
        [_, _, ..] => Err(DefinitionViolation::new(
            DefinitionRule::AmbiguousBodylessResultProvenance,
            result_origin(types, declaration)?,
        )
        .into()),
    }
}

fn explicit(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    receiver: Option<ParameterId>,
    parameters: &[ParameterId],
) -> Result<Option<(CallableProvenance, bool)>, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    // A callable type inside `where` owns its own provenance clause. Declaration provenance is
    // selected only from the declaration's callable tail (or the direct operator/coercion surface)
    // so a nested structural contract cannot silently become the enclosing API contract.
    let clause = syntax::descendant(tree, root, NodeKind::CallableTail)
        .and_then(|tail| syntax::direct_node(tree, tail, NodeKind::ProvenanceClause))
        .or_else(|| syntax::direct_node(tree, root, NodeKind::ProvenanceClause))
        .or_else(|| syntax::direct_node(tree, root, NodeKind::CoercionProvenance));
    let Some(clause) = clause else {
        return Ok(None);
    };
    let tokens: Vec<_> = tree
        .children(clause)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => Some(*token),
            _ => None,
        })
        .skip(1)
        .collect();
    let mut seen = HashMap::new();
    let mut origins = Vec::new();
    let mut includes_static = false;
    for token in tokens {
        let symbol = projection::symbol(types, declaration, token)?;
        if let Some(first) = seen.insert(symbol, token) {
            return Err(DefinitionViolation::duplicate(
                DefinitionRule::DuplicateResultProvenanceOrigin,
                SyntaxOrigin::Token(first),
                SyntaxOrigin::Token(token),
            )
            .into());
        }
        let spelling = token_spelling(types, token)?;
        if spelling == "static" {
            includes_static = true;
            continue;
        }
        if spelling == "self" {
            let receiver = receiver.ok_or_else(|| {
                HeaderDefinitionError::from(DefinitionViolation::new(
                    DefinitionRule::UnknownResultProvenanceOrigin,
                    SyntaxOrigin::Token(token),
                ))
            })?;
            let _ = receiver;
            origins.push(ProvenanceOrigin::Receiver);
            project(
                types,
                declaration,
                SemanticEntity::Parameter(receiver),
                token,
            )?;
            continue;
        }
        let declarations = types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .program
            .declarations();
        let parameter = parameters
            .iter()
            .copied()
            .find(|parameter| {
                declarations
                    .parameter(*parameter)
                    .is_some_and(|item| item.name() == symbol)
            })
            .ok_or_else(|| {
                HeaderDefinitionError::from(DefinitionViolation::new(
                    DefinitionRule::UnknownResultProvenanceOrigin,
                    SyntaxOrigin::Token(token),
                ))
            })?;
        origins.push(ProvenanceOrigin::Parameter(parameter));
        project(
            types,
            declaration,
            SemanticEntity::Parameter(parameter),
            token,
        )?;
    }
    CallableProvenance::from_origins(origins)
        .map(|origins| Some((origins, includes_static)))
        .map_err(|_| HeaderDefinitionError::InvalidProvenance(declaration))
}

fn result_origin(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SyntaxOrigin, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    let tail = syntax::descendant(tree, root, NodeKind::CallableTail);
    let result = tail
        .and_then(|tail| syntax::direct_node(tree, tail, NodeKind::Type))
        .or_else(|| syntax::direct_node(tree, root, NodeKind::BorrowType))
        .or_else(|| syntax::direct_node(tree, root, NodeKind::Type))
        .ok_or(HeaderDefinitionError::MissingCallableResult(declaration))?;
    Ok(SyntaxOrigin::Node(result))
}

fn declared(
    origins: impl IntoIterator<Item = ProvenanceOrigin>,
    declaration: SurfaceDeclarationId,
) -> Result<CallableProvenanceContract, HeaderDefinitionError> {
    CallableProvenance::from_origins(origins)
        .map(CallableProvenanceContract::declared)
        .map_err(|_| HeaderDefinitionError::InvalidProvenance(declaration))
}

fn token_spelling<'a>(
    types: &'a PreparedTypes<'_>,
    token: SyntaxToken,
) -> Result<&'a str, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .and_then(|source| source.text_at(token.range()))
        .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))
}

fn project(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    entity: SemanticEntity,
    token: SyntaxToken,
) -> Result<(), HeaderDefinitionError> {
    projection::token(types, declaration, entity, SourceRole::Reference, token)
}
