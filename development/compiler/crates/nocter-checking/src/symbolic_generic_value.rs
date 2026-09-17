use nocter_declarations::{DeclarationArenas, GenericParameterDomain};
use nocter_model::{GenericParameterId, GenericValue, TypeKind, UsizeTerm};

/// Constructs the identity value of one generic parameter in its declared domain.
///
/// This is the sole checking-side authority for introducing an unspecialized generic value.
/// Consumers cannot assume that every parameter is a type parameter; failure means that the
/// declaration identity or the type transaction is inconsistent with the selected graph.
pub(crate) fn symbolic_generic_value(
    declarations: &DeclarationArenas,
    types: &mut nocter_model::TypeTransaction,
    parameter: GenericParameterId,
) -> Option<GenericValue> {
    let declaration = declarations.generic_parameters().get(parameter)?;
    match declaration.domain() {
        GenericParameterDomain::Type => types
            .intern(TypeKind::GenericParameter(parameter))
            .ok()
            .map(GenericValue::Type),
        GenericParameterDomain::UsizeConstant => {
            Some(GenericValue::Usize(UsizeTerm::Parameter(parameter)))
        }
    }
}
