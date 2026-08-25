use crate::{PreparedTypes, SurfaceDeclarationId};

pub(super) fn gate(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Option<nocter_model::CompilationTarget> {
    let surface = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations[declaration.index()];
    surface.target_gate()
}
