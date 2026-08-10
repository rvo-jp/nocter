pub(super) use super::conformance::{conformed_interface_types, type_satisfies_interface_bound};
use super::model::{Type, TypeEnvironment};
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use std::collections::HashMap;

pub(super) fn interface_symbols_for_generic_parameter<'a>(
    parameter: &str,
    environment: &TypeEnvironment,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, Type)> {
    let Some(requirements) = environment.generic_requirements(parameter) else {
        return Vec::new();
    };
    let substitutions = environment.generic_parameter_substitutions();
    requirements
        .type_bounds()
        .filter_map(|bound| interface_symbol_for_bound(bound, &substitutions, resolved))
        .collect()
}

/// Returns interface contracts known from the lexical predicate environment.
/// Concrete conformances are intentionally handled by `conformed_interface_types`;
/// this function is for types whose capabilities exist only because a declaration
/// constrained them, including associated type projections.
pub(super) fn interface_symbols_for_constrained_type<'a>(
    ty: &Type,
    environment: &TypeEnvironment,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, Type)> {
    match ty {
        Type::Parameter(parameter) => {
            interface_symbols_for_generic_parameter(parameter, environment, resolved)
        }
        Type::Projection { base, member } => {
            interface_symbols_for_constrained_type(base, environment, resolved)
                .into_iter()
                .filter_map(|(owner, interface_type)| {
                    let associated = owner
                        .associated_types
                        .iter()
                        .find(|associated| associated.name == *member)?;
                    let substitutions = type_symbol_substitutions(owner, &interface_type);
                    Some(
                        associated
                            .requirements
                            .type_bounds()
                            .filter_map(|bound| {
                                interface_symbol_for_bound(bound, &substitutions, resolved)
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .flatten()
                .collect()
        }
        _ => Vec::new(),
    }
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
