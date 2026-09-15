use nocter_declarations::{DeclarationGraph, ParameterValueTypeShape};
use nocter_model::{ParameterId, TypeId, TypeKind, TypeTransaction};

/// Interns every effective parameter value type before the prepared type authority is sealed.
///
/// Receiver declarations deliberately store the owning type and borrowing capability separately.
/// This pass closes their derived borrow identities once at the program boundary, so body
/// checking, compile-time projection, and editor consumers cannot make semantic availability
/// depend on whether a particular body happened to mention `self`.
pub(crate) fn prepare_parameter_value_types(
    graph: &DeclarationGraph,
    types: &mut TypeTransaction,
) -> Result<(), (ParameterId, TypeId)> {
    for (parameter, declaration) in graph.declarations().parameters().iter() {
        let value_type = match declaration.value_type_contract() {
            ParameterValueTypeShape::Declared(ty) => types.get(ty).map(|_| ty),
            ParameterValueTypeShape::Borrowed {
                capability,
                referent,
            } => types
                .intern(TypeKind::Borrow {
                    capability,
                    referent,
                })
                .ok(),
        };
        value_type.ok_or((parameter, declaration.ty()))?;
    }
    Ok(())
}
