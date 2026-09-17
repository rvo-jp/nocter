use nocter_declarations::InterfaceApplication;
use nocter_model::{TypeId, TypeStore};

use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, collect_generic_parameters,
    unify_type_pairs,
};

/// Reports whether two normalized interface implementation patterns can denote one concrete application.
///
/// Generic parameter identities are compile-unit global, so variables from two interface implementations
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
    if !constant_arguments_may_match(left_interface, right_interface) {
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

/// Matches a interface implementation pattern against one requested application.
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
        std::iter::once(pattern_target).chain(pattern_interface.arguments().type_values()),
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
    if !bind_pattern_constants(&mut substitution, pattern_interface, requested_interface) {
        return Ok(None);
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
                .zip(right_interface.arguments())
                .filter_map(|(left, right)| match (left, right) {
                    (
                        nocter_model::GenericValue::Type(left),
                        nocter_model::GenericValue::Type(right),
                    ) => Some((*left, *right)),
                    _ => None,
                }),
        )
        .collect()
}

fn constant_arguments_may_match(left: &InterfaceApplication, right: &InterfaceApplication) -> bool {
    let mut bindings = std::collections::HashMap::new();
    left.arguments()
        .iter()
        .zip(right.arguments())
        .all(|(left, right)| match (left, right) {
            (nocter_model::GenericValue::Usize(left), nocter_model::GenericValue::Usize(right)) => {
                unify_usize_terms(*left, *right, &mut bindings)
            }
            (nocter_model::GenericValue::Type(_), nocter_model::GenericValue::Type(_)) => true,
            _ => false,
        })
}

fn unify_usize_terms(
    left: nocter_model::UsizeTerm,
    right: nocter_model::UsizeTerm,
    bindings: &mut std::collections::HashMap<
        nocter_model::GenericParameterId,
        nocter_model::UsizeTerm,
    >,
) -> bool {
    let left = resolve_usize_term(left, bindings);
    let right = resolve_usize_term(right, bindings);
    match (left, right) {
        (nocter_model::UsizeTerm::Value(left), nocter_model::UsizeTerm::Value(right)) => {
            left == right
        }
        (nocter_model::UsizeTerm::Parameter(left), nocter_model::UsizeTerm::Parameter(right))
            if left == right =>
        {
            true
        }
        (nocter_model::UsizeTerm::Parameter(parameter), value)
        | (value, nocter_model::UsizeTerm::Parameter(parameter)) => {
            bindings.insert(parameter, value);
            true
        }
    }
}

fn resolve_usize_term(
    mut term: nocter_model::UsizeTerm,
    bindings: &std::collections::HashMap<nocter_model::GenericParameterId, nocter_model::UsizeTerm>,
) -> nocter_model::UsizeTerm {
    let mut visited = std::collections::HashSet::new();
    while let nocter_model::UsizeTerm::Parameter(parameter) = term {
        if !visited.insert(parameter) {
            break;
        }
        let Some(next) = bindings.get(&parameter).copied() else {
            break;
        };
        term = next;
    }
    term
}

fn bind_pattern_constants(
    substitution: &mut TypeSubstitution,
    pattern: &InterfaceApplication,
    requested: &InterfaceApplication,
) -> bool {
    let mut bindings = std::collections::HashMap::new();
    for (pattern, requested) in pattern.arguments().iter().zip(requested.arguments()) {
        match (pattern, requested) {
            (
                nocter_model::GenericValue::Usize(nocter_model::UsizeTerm::Parameter(parameter)),
                nocter_model::GenericValue::Usize(requested),
            ) => {
                if bindings
                    .insert(*parameter, *requested)
                    .is_some_and(|value| value != *requested)
                {
                    return false;
                }
            }
            (
                nocter_model::GenericValue::Usize(nocter_model::UsizeTerm::Value(pattern)),
                nocter_model::GenericValue::Usize(nocter_model::UsizeTerm::Value(requested)),
            ) if pattern == requested => {}
            (nocter_model::GenericValue::Type(_), nocter_model::GenericValue::Type(_)) => {}
            (nocter_model::GenericValue::Usize(_), nocter_model::GenericValue::Usize(_)) => {
                return false;
            }
            _ => return false,
        }
    }
    for (parameter, value) in bindings {
        substitution.bind_constant_term(parameter, value);
    }
    true
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
    use nocter_declarations::InterfaceApplication;
    use nocter_model::{
        ArenaBuilder, GenericApplication, GenericParameterId, GenericValue, InterfaceId,
        TypeAuthority, UsizeTerm,
    };

    use super::{match_pattern, patterns_overlap};

    fn identities() -> (InterfaceId, GenericParameterId) {
        let mut interfaces = ArenaBuilder::<InterfaceId, _>::new();
        let interface = interfaces.insert(());
        let _ = interfaces.finish();
        let mut parameters = ArenaBuilder::<GenericParameterId, _>::new();
        let parameter = parameters.insert(());
        let _ = parameters.finish();
        (interface, parameter)
    }

    #[test]
    fn repeated_constant_pattern_does_not_overlap_conflicting_values() {
        let (interface, parameter) = identities();
        let types = TypeAuthority::new();
        let target = types.store().builtin(nocter_model::BuiltinType::I32);
        let pattern = InterfaceApplication::new(
            interface,
            GenericApplication::new([
                GenericValue::Usize(UsizeTerm::Parameter(parameter)),
                GenericValue::Usize(UsizeTerm::Parameter(parameter)),
            ]),
        );
        let conflicting = InterfaceApplication::new(
            interface,
            GenericApplication::new([
                GenericValue::Usize(UsizeTerm::Value(1)),
                GenericValue::Usize(UsizeTerm::Value(2)),
            ]),
        );

        assert_eq!(
            patterns_overlap(types.store(), &pattern, target, &conflicting, target),
            Ok(false)
        );
    }

    #[test]
    fn matching_constant_pattern_produces_a_substitution() {
        let (interface, parameter) = identities();
        let mut types = TypeAuthority::new().transaction();
        let target = types.builtin(nocter_model::BuiltinType::I32);
        let pattern = InterfaceApplication::new(
            interface,
            GenericApplication::new([GenericValue::Usize(UsizeTerm::Parameter(parameter))]),
        );
        let requested = InterfaceApplication::new(
            interface,
            GenericApplication::new([GenericValue::Usize(UsizeTerm::Value(8))]),
        );
        let substitution = match_pattern(&types, &pattern, target, &requested, target)
            .unwrap()
            .expect("constant pattern should match");

        assert_eq!(
            substitution
                .apply_application(&mut types, pattern.arguments())
                .unwrap(),
            requested.arguments().clone()
        );
    }
}
