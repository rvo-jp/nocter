use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::HeaderDefinitionError;

pub(super) fn gate(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<Option<nocter_model::CompilationTarget>, HeaderDefinitionError> {
    let surface = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations[declaration.index()];
    Ok(surface.target_gate())
}
