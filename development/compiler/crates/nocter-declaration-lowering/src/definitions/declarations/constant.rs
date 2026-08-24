use nocter_declarations::ConstantDeclaration;
use nocter_syntax::NodeKind;

use crate::{PreparedTypes, SurfaceDeclarationId};

use super::super::{HeaderDefinitionError, projection, syntax};
use super::{name, site, target};

/// Freezes the values selected by the header-constant pass into declaration storage.
///
/// Initializer syntax is intentionally unavailable here: fixed-array normalization and ordinary
/// value use must consume the exact same already-evaluated authority.
pub(super) fn define_all(types: &mut PreparedTypes<'_>) -> Result<(), HeaderDefinitionError> {
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
            prepared.value,
            target::gate(types, declaration)?,
        );
        types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .program
            .declarations_mut()
            .define_constant(id, definition)?;
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
