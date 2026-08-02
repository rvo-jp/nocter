use super::model::{Type, TypeEnvironment};
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use std::collections::HashMap;

pub(super) fn interface_symbol_for_generic_parameter<'a>(
    parameter: &str,
    environment: &TypeEnvironment,
    resolved: &'a ResolveOutput,
) -> Option<(&'a TypeSymbol, Type)> {
    let bound = environment.generic_bound(parameter)?;
    interface_symbol_for_bound(
        bound,
        &environment.generic_parameter_substitutions(),
        resolved,
    )
}

pub(super) fn interface_symbol_for_bound<'a>(
    bound: &TypeExpr,
    substitutions: &HashMap<String, Type>,
    resolved: &'a ResolveOutput,
) -> Option<(&'a TypeSymbol, Type)> {
    let bound_type = type_expr_to_type_with_substitutions(bound, resolved, None, substitutions);
    let symbol = resolved.type_symbol_by_canonical_name(bound_type.nominal_name()?)?;
    (symbol.kind == TypeSymbolKind::Interface).then_some((symbol, bound_type))
}

pub(super) fn type_satisfies_interface_bound(
    actual: &Type,
    bound: &Type,
    resolved: &ResolveOutput,
) -> bool {
    if actual == bound {
        return true;
    }
    let Some(actual_symbol) = actual
        .nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
    else {
        return false;
    };
    let substitutions = type_symbol_substitutions(actual_symbol, actual);
    actual_symbol.interface_impls.iter().any(|implemented| {
        type_expr_to_type_with_substitutions(implemented, resolved, None, &substitutions) == *bound
    })
}

pub(super) fn implemented_interface_types(actual: &Type, resolved: &ResolveOutput) -> Vec<Type> {
    let Some(symbol) = actual
        .nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
    else {
        return Vec::new();
    };
    let substitutions = type_symbol_substitutions(symbol, actual);
    symbol
        .interface_impls
        .iter()
        .map(|implemented| {
            type_expr_to_type_with_substitutions(implemented, resolved, None, &substitutions)
        })
        .collect()
}

pub(super) fn type_symbol_substitutions(symbol: &TypeSymbol, ty: &Type) -> HashMap<String, Type> {
    let Type::Generic { name, arguments } = ty else {
        return HashMap::new();
    };
    if name != &symbol.canonical_name {
        return HashMap::new();
    }
    symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect()
}
