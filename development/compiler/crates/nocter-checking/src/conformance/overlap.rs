use nocter_declarations::InterfaceApplication;
use nocter_model::{TypeId, TypeStore};

use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, collect_generic_parameters,
    unify_type_pairs,
};

/// Reports whether two normalized conformance patterns can denote one concrete application.
///
/// Generic parameter identities are compile-unit global, so variables from two conformances
/// cannot alias accidentally. Non-refinement requirements do not make a pattern disjoint: a
/// concrete type may satisfy both sets of capabilities.
pub(super) fn patterns_overlap(
    types: &TypeStore,
    left_interface: &InterfaceApplication,
    left_target: TypeId,
    right_interface: &InterfaceApplication,
    right_target: TypeId,
) -> Result<bool, SubstitutionError> {
    if left_interface.interface() != right_interface.interface()
        || left_interface.arguments().len() != right_interface.arguments().len()
    {
        return Ok(false);
    }
    let equations =
        application_equations(left_interface, left_target, right_interface, right_target);
    let variables = collect_generic_parameters(
        types,
        equations.iter().flat_map(|(left, right)| [*left, *right]),
    )
    .map_err(invalid_unification)?;
    match unify_type_pairs(types, variables, equations) {
        Ok(_) => Ok(true),
        Err(TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. }) => {
            Ok(false)
        }
        Err(error) => Err(invalid_unification(error)),
    }
}

/// Matches a conformance pattern against one requested application.
///
/// Only generic parameters reachable from the pattern are variables. Requested generic
/// parameters remain opaque even when a repeated pattern binding causes one to appear on the left
/// side of a later equation.
pub(super) fn match_pattern(
    types: &TypeStore,
    pattern_interface: &InterfaceApplication,
    pattern_target: TypeId,
    requested_interface: &InterfaceApplication,
    requested_target: TypeId,
) -> Result<Option<TypeSubstitution>, SubstitutionError> {
    if pattern_interface.interface() != requested_interface.interface()
        || pattern_interface.arguments().len() != requested_interface.arguments().len()
    {
        return Ok(None);
    }
    let equations = application_equations(
        pattern_interface,
        pattern_target,
        requested_interface,
        requested_target,
    );
    let variables = collect_generic_parameters(
        types,
        std::iter::once(pattern_target).chain(pattern_interface.arguments().iter().copied()),
    )
    .map_err(invalid_unification)?;
    let bindings = match unify_type_pairs(types, variables, equations) {
        Ok(bindings) => bindings,
        Err(TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. }) => {
            return Ok(None);
        }
        Err(error) => return Err(invalid_unification(error)),
    };
    let mut substitution = TypeSubstitution::default();
    for (parameter, ty) in bindings.iter() {
        substitution.bind_generic(parameter, ty);
    }
    Ok(Some(substitution))
}

fn application_equations(
    left_interface: &InterfaceApplication,
    left_target: TypeId,
    right_interface: &InterfaceApplication,
    right_target: TypeId,
) -> Vec<(TypeId, TypeId)> {
    std::iter::once((left_target, right_target))
        .chain(
            left_interface
                .arguments()
                .iter()
                .copied()
                .zip(right_interface.arguments().iter().copied()),
        )
        .collect()
}

fn invalid_unification(error: TypeUnificationError) -> SubstitutionError {
    match error {
        TypeUnificationError::UnknownType(ty) => SubstitutionError::UnknownType(ty),
        TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. } => {
            SubstitutionError::InvalidStore
        }
    }
}
