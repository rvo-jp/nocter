use nocter_model::Symbol;
use nocter_syntax::{NodeKind, SyntaxToken, TokenKind};

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::allocation::surface_node;
use super::super::{HeaderDefinitionError, projection, syntax};

pub(super) fn gate(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<Option<Symbol>, HeaderDefinitionError> {
    let surface = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations[declaration.index()];
    let Some(gate) = surface.target_gate() else {
        return Ok(None);
    };
    let tree = projection::tree(types, declaration)?;
    let literal = syntax::descendant(tree, gate, NodeKind::StringLiteral)
        .ok_or(HeaderDefinitionError::InvalidTargetGate(declaration))?;
    let source = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(tree.source())
        .ok_or(HeaderDefinitionError::MissingSource(declaration))?;
    let decoded = crate::text::decode_string_literal(source, tree, literal)
        .ok_or(HeaderDefinitionError::InvalidTargetGate(declaration))?;
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .symbols()
        .get(&decoded)
        .map(Some)
        .ok_or(HeaderDefinitionError::InvalidTargetGate(declaration))
}

pub(super) fn contextual_word(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    word: &str,
) -> Result<Option<SyntaxToken>, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let source = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(tree.source())
        .ok_or(HeaderDefinitionError::MissingSource(declaration))?;
    Ok(
        syntax::direct_tokens(tree, surface_node(types, declaration)?)
            .into_iter()
            .filter(|token| token.kind() == TokenKind::Identifier)
            .find(|token| source.text_at(token.range()) == Some(word)),
    )
}
