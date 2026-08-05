pub(super) use super::conformance::{implemented_interface_types, type_satisfies_interface_bound};
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
    let Some(bounds) = environment.generic_bounds(parameter) else {
        return Vec::new();
    };
    let substitutions = environment.generic_parameter_substitutions();
    bounds
        .iter()
        .filter_map(|bound| interface_symbol_for_bound(bound, &substitutions, resolved))
        .collect()
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
