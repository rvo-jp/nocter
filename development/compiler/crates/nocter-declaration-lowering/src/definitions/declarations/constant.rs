use nocter_declarations::ConstantDeclaration;
use nocter_syntax::NodeKind;

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::{HeaderDefinitionError, allocation::AllocatedHeaders, projection, syntax};
use super::{name, site, target};

/// Freezes metadata selected by the header-constant pass before initializer values are attached.
///
/// Initializer syntax is intentionally unavailable here: fixed-array normalization and ordinary
/// value use must consume the exact same already-evaluated authority. Value completion belongs to
/// the later consuming program transition, not declaration definition.
pub(super) fn define_all(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
) -> Result<(), HeaderDefinitionError> {
    let mut values = types
        .constant_values
        .iter()
        .map(|(id, prepared)| (*id, prepared.clone()))
        .collect::<Vec<_>>();
    values.sort_unstable_by_key(|(id, _)| *id);
    for (id, prepared) in values {
        let declaration = prepared.declaration;
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
    }
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
