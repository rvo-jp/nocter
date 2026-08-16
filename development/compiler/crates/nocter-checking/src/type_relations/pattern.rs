use nocter_model::{TypeId, TypeStore};

use super::{
    GenericBindings, SubstitutionError, TypeUnificationError, collect_generic_parameters,
    unify_type_pairs,
};

/// Reports whether two normalized declaration type patterns can denote one concrete type.
pub(crate) fn type_patterns_overlap(
    types: &TypeStore,
    left: TypeId,
    right: TypeId,
) -> Result<bool, SubstitutionError> {
    let variables =
        collect_generic_parameters(types, [left, right]).map_err(invalid_unification)?;
    match unify_type_pairs(types, variables, [(left, right)]) {
        Ok(_) => Ok(true),
        Err(TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. }) => {
            Ok(false)
        }
        Err(error) => Err(invalid_unification(error)),
    }
}

/// Matches one normalized declaration pattern against a requested type.
///
/// Only generic identities reachable from `pattern` are variables. Generic identities in the
/// requested type remain opaque caller-owned terms.
pub(crate) fn match_type_pattern(
    types: &TypeStore,
    pattern: TypeId,
    requested: TypeId,
) -> Result<Option<GenericBindings>, SubstitutionError> {
    let variables = collect_generic_parameters(types, [pattern]).map_err(invalid_unification)?;
    match unify_type_pairs(types, variables, [(pattern, requested)]) {
        Ok(bindings) => Ok(Some(bindings)),
        Err(TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. }) => {
            Ok(None)
        }
        Err(error) => Err(invalid_unification(error)),
    }
}

fn invalid_unification(error: TypeUnificationError) -> SubstitutionError {
    match error {
        TypeUnificationError::UnknownType(ty) => SubstitutionError::UnknownType(ty),
        TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. } => {
            SubstitutionError::InvalidStore
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, GenericParameterId, TypeKind, TypeStore};

    use super::{match_type_pattern, type_patterns_overlap};

    #[test]
    fn normalized_refinements_are_disjoint_but_generic_patterns_overlap() {
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        let mut types = TypeStore::new();
        let generic = types.intern(TypeKind::GenericParameter(parameter)).unwrap();
        let i32 = types.builtin(BuiltinType::I32);
        let u32 = types.builtin(BuiltinType::U32);

        assert!(type_patterns_overlap(&types, generic, i32).unwrap());
        assert!(!type_patterns_overlap(&types, i32, u32).unwrap());
        assert!(match_type_pattern(&types, generic, u32).unwrap().is_some());
    }
}
