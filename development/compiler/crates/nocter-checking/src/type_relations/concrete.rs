use std::collections::HashSet;

use nocter_model::{TypeId, TypeKind, TypeStore};

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
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match types.get(ty).ok_or(SubstitutionError::UnknownType(ty))? {
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => return Ok(false),
            TypeKind::Builtin(_) => {}
            TypeKind::Nominal { arguments, .. }
            | TypeKind::Opaque { arguments, .. }
            | TypeKind::Closure { arguments, .. } => {
                pending.extend(arguments.iter().copied());
            }
            TypeKind::Pointer(base)
            | TypeKind::Borrow { referent: base, .. }
            | TypeKind::Slice(base)
            | TypeKind::FixedArray { element: base, .. }
            | TypeKind::Optional(base)
            | TypeKind::Fallible(base) => pending.push(*base),
            TypeKind::Callable(contract) => {
                pending.push(contract.result());
                pending.extend(contract.parameters().iter().copied());
                pending.extend(contract.pack());
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, GenericParameterId, TypeKind, TypeStore};

    use super::is_concrete_type;

    #[test]
    fn nested_generic_prevents_concrete_selection() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let generic = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let generic_array = types
            .intern(TypeKind::FixedArray {
                element: generic,
                length: 1,
            })
            .unwrap();
        let concrete_array = types
            .intern(TypeKind::FixedArray {
                element: types.builtin(BuiltinType::I32),
                length: 1,
            })
            .unwrap();

        assert!(!is_concrete_type(&types, generic_array).unwrap());
        assert!(is_concrete_type(&types, concrete_array).unwrap());
    }
}
