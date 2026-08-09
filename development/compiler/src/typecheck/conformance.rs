use super::model::Type;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use crate::resolve::{InterfaceConformance, ResolveOutput};
use std::collections::{HashMap, HashSet};

pub(super) fn implemented_interface_types(actual: &Type, resolved: &ResolveOutput) -> Vec<Type> {
    let mut active = HashSet::new();
    implemented_interface_types_inner(actual, resolved, &mut active)
        .into_iter()
        .map(|(_, interface)| interface)
        .collect()
}

pub(super) fn implemented_interface_conformances<'a>(
    actual: &Type,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a InterfaceConformance, Type)> {
    let mut active = HashSet::new();
    implemented_interface_types_inner(actual, resolved, &mut active)
}

pub(super) fn type_satisfies_interface_bound(
    actual: &Type,
    bound: &Type,
    resolved: &ResolveOutput,
) -> bool {
    if actual == bound {
        return true;
    }
    let mut active = HashSet::new();
    satisfies_inner(actual, bound, resolved, &mut active)
}

fn satisfies_inner(
    actual: &Type,
    bound: &Type,
    resolved: &ResolveOutput,
    active: &mut HashSet<(String, String)>,
) -> bool {
    if let Type::Closure(closure) = actual
        && super::closures::closure_satisfies_callable_bound(closure, bound, resolved)
    {
        return true;
    }
    let key = (actual.display(), bound.display());
    if !active.insert(key.clone()) {
        return false;
    }
    let satisfied = implemented_interface_types_inner(actual, resolved, active)
        .into_iter()
        .any(|(_, implemented)| implemented == *bound);
    active.remove(&key);
    satisfied
}

fn implemented_interface_types_inner<'a>(
    actual: &Type,
    resolved: &'a ResolveOutput,
    active: &mut HashSet<(String, String)>,
) -> Vec<(&'a InterfaceConformance, Type)> {
    let Some(symbol) = actual
        .nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
    else {
        return Vec::new();
    };

    symbol
        .interface_conformances
        .iter()
        .filter_map(|conformance| {
            specialize_conformance(conformance, actual, resolved, active)
                .map(|interface| (conformance, interface))
        })
        .collect()
}

fn specialize_conformance(
    conformance: &InterfaceConformance,
    actual: &Type,
    resolved: &ResolveOutput,
    active: &mut HashSet<(String, String)>,
) -> Option<Type> {
    let parameters = conformance
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_type_expr_substitutions(
        &conformance.target_ty,
        actual,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    let specialized_target = type_expr_to_type_with_substitutions(
        &conformance.target_ty,
        resolved,
        None,
        &substitutions,
    );
    if specialized_target != *actual {
        return None;
    }

    for (parameter, bounds) in conformance
        .generic_parameters
        .iter()
        .zip(&conformance.generic_parameter_requirements)
    {
        let parameter_type = substitutions.get(parameter)?;
        if bounds.has_copy() && !super::copyability::type_is_copy(parameter_type, resolved) {
            return None;
        }
        for bound in bounds.type_bounds() {
            let bound_type =
                type_expr_to_type_with_substitutions(bound, resolved, None, &substitutions);
            if bound_type.is_unknown_or_unresolved()
                || !satisfies_inner(parameter_type, &bound_type, resolved, active)
            {
                return None;
            }
        }
    }

    let interface = type_expr_to_type_with_substitutions(
        &conformance.interface_ty,
        resolved,
        None,
        &substitutions,
    );
    (!interface.is_unknown_or_unresolved()).then_some(interface)
}
