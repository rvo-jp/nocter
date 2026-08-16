use nocter_model::Symbol;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::{NodeId, SyntaxToken, SyntaxTree};

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::HeaderDefinitionError;

pub(super) fn tree<'a>(
    types: &'a PreparedTypes<'a>,
    declaration: SurfaceDeclarationId,
) -> Result<&'a SyntaxTree, HeaderDefinitionError> {
    let reserved = &types.namespaces.imports.generics.headers.reserved;
    let surface = reserved
        .declarations
        .get(declaration.index())
        .ok_or(HeaderDefinitionError::MissingSource(declaration))?;
    reserved
        .sources
        .get(surface.source().index())
        .map(crate::SurfaceSource::syntax)
        .ok_or(HeaderDefinitionError::MissingSource(declaration))
}

pub(super) fn symbol(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    token: SyntaxToken,
) -> Result<Symbol, HeaderDefinitionError> {
    let reserved = &types.namespaces.imports.generics.headers.reserved;
    let source = reserved
        .source_map
        .get(token.source())
        .ok_or(HeaderDefinitionError::MissingSource(declaration))?;
    let spelling = source
        .text_at(token.range())
        .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))?;
    reserved
        .program
        .symbols()
        .get(spelling)
        .ok_or(HeaderDefinitionError::MissingName(declaration))
}

pub(super) fn node(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    entity: SemanticEntity,
    role: SourceRole,
    node: NodeId,
) -> Result<(), HeaderDefinitionError> {
    let origin = SourceOrigin::from_node(tree(types, declaration)?, node)
        .map_err(|_| HeaderDefinitionError::InconsistentSource(node.source()))?;
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_index
        .insert(entity, role, origin)?;
    Ok(())
}

pub(super) fn token(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    entity: SemanticEntity,
    role: SourceRole,
    token: SyntaxToken,
) -> Result<(), HeaderDefinitionError> {
    let origin = SourceOrigin::from_token(tree(types, declaration)?, token)
        .map_err(|_| HeaderDefinitionError::InconsistentSource(token.source()))?;
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_index
        .insert(entity, role, origin)?;
    Ok(())
}

pub(super) fn role(types: &PreparedTypes<'_>, declaration: SurfaceDeclarationId) -> SourceRole {
    if types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .contracts
        .is_implementation(declaration)
    {
        SourceRole::Implementation
    } else {
        SourceRole::Declaration
    }
}
