use nocter_model::Symbol;
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{SyntaxToken, SyntaxTree};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::{TypeBindingError, TypeBindingRule};

pub(super) fn declaration_module(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::ModuleId, TypeBindingError> {
    let reserved = &namespaces.imports.generics.headers.reserved;
    let surface = reserved
        .declarations
        .get(declaration.index())
        .ok_or(TypeBindingError::MissingSource(declaration))?;
    reserved
        .module_for_source(surface.source())
        .ok_or(TypeBindingError::MissingSource(declaration))
}

pub(super) fn declaration_source(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<crate::SurfaceSourceId, TypeBindingError> {
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .map(|surface| surface.source())
        .ok_or(TypeBindingError::MissingSource(declaration))
}

pub(super) fn require_arity(
    namespaces: &PreparedNamespaces<'_>,
    origin: SyntaxOrigin,
    entity: ReservedEntity,
    actual: usize,
) -> Result<(), TypeBindingError> {
    let generics = &namespaces.imports.generics;
    let expected = generics
        .headers
        .reserved
        .declaration_for_entity(entity)
        .and_then(|declaration| generics.own.get(declaration.index()))
        .map_or(0, |parameters| parameters.len());
    if expected == actual {
        Ok(())
    } else {
        Err(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeArguments,
            origin,
        ))
    }
}

pub(super) fn token_symbol(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<Symbol, TypeBindingError> {
    let text = token_text(namespaces, tree, token)?;
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .symbols()
        .get(text)
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
}

pub(super) fn token_text<'a>(
    namespaces: &'a PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<&'a str, TypeBindingError> {
    let source = namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))?;
    source
        .text_at(token.range())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
}
