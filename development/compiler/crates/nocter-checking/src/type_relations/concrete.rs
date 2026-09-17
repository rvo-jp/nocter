use nocter_model::{GenericValue, TypeId, TypeStore};

use super::SubstitutionError;

/// Returns whether a type is closed over concrete semantic identities.
///
/// Body checking cannot select a concrete declaration for a type that still contains a lexical
/// generic, interface `Self`, or associated projection. Those operations require exact lexical
/// evidence and are resolved only after executable specialization supplies concrete types.
///
/// # Errors
///
/// Returns [`SubstitutionError::UnknownType`] when the root or a referenced type is absent from
/// the supplied store.
pub fn is_concrete_type(types: &TypeStore, root: TypeId) -> Result<bool, SubstitutionError> {
    types
        .is_concrete(root)
        .ok_or(SubstitutionError::UnknownType(root))
}

/// Returns whether one normalized generic value is closed for executable specialization.
///
/// Type and constant arguments share the same ordered identity, so downstream stages must use
/// this domain-aware predicate instead of assuming every argument is a type.
///
/// # Errors
///
/// Returns [`SubstitutionError::UnknownType`] when a type argument or one of its referenced types
/// is absent from the supplied store.
pub fn is_concrete_generic_value(
    types: &TypeStore,
    value: GenericValue,
) -> Result<bool, SubstitutionError> {
    match value {
        GenericValue::Type(ty) => is_concrete_type(types, ty),
        GenericValue::Usize(value) => Ok(value.closed_value().is_some()),
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, GenericParameterId, TypeAuthority, TypeKind};

    use super::is_concrete_type;

    #[test]
    fn nested_generic_prevents_concrete_selection() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeAuthority::new().transaction();
        let generic = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let generic_array = types
            .intern(TypeKind::FixedArray {
                element: generic,
                length: 1.into(),
            })
            .unwrap();
        let concrete_array = types
            .intern(TypeKind::FixedArray {
                element: types.builtin(BuiltinType::I32),
                length: 1.into(),
            })
            .unwrap();

        assert!(!is_concrete_type(&types, generic_array).unwrap());
        assert!(is_concrete_type(&types, concrete_array).unwrap());
    }
}
