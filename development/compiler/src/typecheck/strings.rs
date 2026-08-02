use super::diagnostics::interpolated_string_part_type_unsupported_diagnostic;
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_bool_type;
use crate::ast::{InterpolatedStringExpr, InterpolatedStringPart};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn interpolated_string_type(resolved: &ResolveOutput) -> Type {
    string_type(resolved)
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

        if interpolation_input_kind(&actual, resolved).is_none() {
            diagnostics.push(interpolated_string_part_type_unsupported_diagnostic(
                sources, part, &actual,
            ));
        }
    }
}

fn string_type(resolved: &ResolveOutput) -> Type {
    runtime_string_symbol(resolved)
        .map(|symbol| Type::Named(symbol.canonical_name.clone()))
        .unwrap_or_else(|| Type::Unresolved("String".to_string()))
}

pub(super) fn interpolation_input_kind(
    ty: &Type,
    resolved: &ResolveOutput,
) -> Option<crate::semantics::InterpolationInputKind> {
    use crate::semantics::InterpolationInputKind;

    if matches!(ty, Type::Str) {
        return Some(InterpolationInputKind::Str);
    }
    if matches!(ty, Type::I32) {
        return Some(InterpolationInputKind::I32);
    }
    if matches!(ty, Type::Primitive(name) if name == "u8") {
        return Some(InterpolationInputKind::U8);
    }
    if matches!(ty, Type::Primitive(name) if name == "usize") {
        return Some(InterpolationInputKind::Usize);
    }
    if is_bool_type(ty) {
        return Some(InterpolationInputKind::Bool);
    }
    is_string_type(ty, resolved).then_some(InterpolationInputKind::String)
}

fn is_string_type(ty: &Type, resolved: &ResolveOutput) -> bool {
    let Some(canonical_name) = ty.nominal_name() else {
        return false;
    };

    runtime_string_symbol(resolved).is_some_and(|symbol| symbol.canonical_name == canonical_name)
}

fn runtime_string_symbol(resolved: &ResolveOutput) -> Option<&crate::resolve::TypeSymbol> {
    let declaration = resolved
        .trusted_declarations
        .interpolation_runtime()?
        .string_type_declaration;
    resolved.symbols.symbols().find_map(|symbol| {
        let crate::resolve::SymbolKind::Type(type_symbol) = &symbol.kind else {
            return None;
        };
        (symbol.declaration_span == declaration).then_some(type_symbol)
    })
}
