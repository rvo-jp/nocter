use super::diagnostics::interpolated_string_part_type_unsupported_diagnostic;
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::numeric::is_integer_type;
use super::operations::is_bool_type;
use crate::ast::{InterpolatedStringExpr, InterpolatedStringPart};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn interpolated_string_type(resolved: &ResolveOutput) -> Type {
    Type::Fallible {
        success: Box::new(string_type(resolved)),
        error: Box::new(Type::Error),
    }
}

pub(super) fn check_interpolated_string_expression(
    sources: &SourceMap,
    expression: &InterpolatedStringExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    for part in &expression.parts {
        let InterpolatedStringPart::Expression(part) = part else {
            continue;
        };
        let actual = expression_type(&part.expression, resolved, environment);
        if actual.is_unknown_or_unresolved() {
            continue;
        }

        if !is_supported_interpolation_type(&actual, resolved) {
            diagnostics.push(interpolated_string_part_type_unsupported_diagnostic(
                sources, part, &actual,
            ));
        }
    }
}

fn string_type(resolved: &ResolveOutput) -> Type {
    resolved
        .type_symbol_by_name("String")
        .map(|symbol| Type::Named(symbol.canonical_name.clone()))
        .unwrap_or_else(|| Type::Unresolved("String".to_string()))
}

fn is_supported_interpolation_type(ty: &Type, resolved: &ResolveOutput) -> bool {
    matches!(ty, Type::Str)
        || is_integer_type(ty)
        || is_bool_type(ty)
        || is_string_type(ty, resolved)
}

fn is_string_type(ty: &Type, resolved: &ResolveOutput) -> bool {
    let Some(canonical_name) = ty.nominal_name() else {
        return false;
    };

    resolved
        .type_symbol_by_name("String")
        .is_some_and(|symbol| symbol.canonical_name == canonical_name)
}
