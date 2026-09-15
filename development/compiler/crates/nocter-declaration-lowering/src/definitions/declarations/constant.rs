use nocter_declarations::ConstantDeclaration;
use nocter_syntax::NodeKind;

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::{HeaderDefinitionError, allocation::AllocatedHeaders, projection, syntax};
use super::{name, site, target};

/// Freezes one constant's metadata independently of structural-value eligibility.
pub(super) fn define(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::ConstantId,
) -> Result<(), HeaderDefinitionError> {
    let ty = constant_type(types, declaration)?;
    let definition = ConstantDeclaration::new(
        site(types, declaration)?,
        name(types, declaration)?,
        ty,
        allocated.bodies[declaration.index()]
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?,
        target::gate(types, declaration),
    );
    let program = &mut types.namespaces.imports.generics.headers.reserved.program;
    program.define_constant_metadata(id, definition)?;
    Ok(())
}

fn constant_type(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::TypeId, HeaderDefinitionError> {
    let reserved = &types.namespaces.imports.generics.headers.reserved;
    let surface = reserved.declarations[declaration.index()];
    let tree = projection::tree(types, declaration)?;
    let node = syntax::direct_node(tree, surface.node(), NodeKind::Type)
        .ok_or(HeaderDefinitionError::MissingType(surface.node()))?;
    types
        .roots
        .get(&node)
        .copied()
        .ok_or(HeaderDefinitionError::MissingType(node))
}
