use nocter_declarations::InterfaceApplication;
use nocter_model::{TypeId, TypeStore};

use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, collect_generic_parameters,
    unify_type_and_constant_pairs,
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
    let Some((equations, constant_equations)) =
        application_equations(left_interface, left_target, right_interface, right_target)
    else {
        return Ok(false);
    };
    let mut variables = collect_generic_parameters(
        types,
        equations.iter().flat_map(|(left, right)| [*left, *right]),
    )
    .map_err(invalid_unification)?;
    extend_constant_parameters(&mut variables, left_interface);
    extend_constant_parameters(&mut variables, right_interface);
    match unify_type_and_constant_pairs(types, variables, equations, constant_equations) {
        Ok(_) => Ok(true),
        Err(
            TypeUnificationError::Conflict(_)
            | TypeUnificationError::ConstantConflict { .. }
            | TypeUnificationError::RecursiveBinding { .. },
        ) => Ok(false),
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
    let Some((equations, constant_equations)) = application_equations(
        pattern_interface,
        pattern_target,
        requested_interface,
        requested_target,
    ) else {
        return Ok(None);
    };
    let mut variables = collect_generic_parameters(
        types,
        std::iter::once(pattern_target).chain(pattern_interface.arguments().type_values()),
    )
    .map_err(invalid_unification)?;
    extend_constant_parameters(&mut variables, pattern_interface);
    let bindings =
        match unify_type_and_constant_pairs(types, variables, equations, constant_equations) {
            Ok(bindings) => bindings,
            Err(
                TypeUnificationError::Conflict(_)
                | TypeUnificationError::ConstantConflict { .. }
                | TypeUnificationError::RecursiveBinding { .. },
            ) => {
                return Ok(None);
            }
            Err(error) => return Err(invalid_unification(error)),
        };
    let mut substitution = TypeSubstitution::default();
    for (parameter, value) in bindings.values() {
        substitution.bind_value(parameter, value);
    }
    Ok(Some(substitution))
}

fn application_equations(
    left_interface: &InterfaceApplication,
    left_target: TypeId,
    right_interface: &InterfaceApplication,
    right_target: TypeId,
) -> Option<(
    Vec<(TypeId, TypeId)>,
    Vec<(nocter_model::UsizeTerm, nocter_model::UsizeTerm)>,
)> {
    let mut types = vec![(left_target, right_target)];
    let mut constants = Vec::new();
    for (left, right) in left_interface
        .arguments()
        .iter()
        .zip(right_interface.arguments())
    {
        match (left, right) {
            (nocter_model::GenericValue::Type(left), nocter_model::GenericValue::Type(right)) => {
                types.push((*left, *right))
            }
            (nocter_model::GenericValue::Usize(left), nocter_model::GenericValue::Usize(right)) => {
                constants.push((*left, *right))
            }
            _ => return None,
        }
    }
    Some((types, constants))
}

fn extend_constant_parameters(
    output: &mut std::collections::HashSet<nocter_model::GenericParameterId>,
    application: &InterfaceApplication,
) {
    output.extend(
        application
            .arguments()
            .iter()
            .filter_map(|value| match value {
                nocter_model::GenericValue::Usize(nocter_model::UsizeTerm::Parameter(
                    parameter,
                )) => Some(*parameter),
                _ => None,
            }),
    );
}

fn invalid_unification(error: TypeUnificationError) -> SubstitutionError {
    match error {
        TypeUnificationError::UnknownType(ty) => SubstitutionError::UnknownType(ty),
        TypeUnificationError::Conflict(_)
        | TypeUnificationError::ConstantConflict { .. }
        | TypeUnificationError::RecursiveBinding { .. } => SubstitutionError::InvalidStore,
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
