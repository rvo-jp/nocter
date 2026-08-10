use super::calls::method_self_type_for_receiver;
use super::diagnostics::{
    interpolated_string_part_type_unsupported_diagnostic,
    interpolation_runtime_unavailable_diagnostic,
};
use super::expressions::expression_type;
use super::facts::TypecheckProtocolMethod;
use super::model::{Type, TypeEnvironment};
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
    if resolved
        .trusted_declarations
        .interpolation_runtime()
        .is_none()
    {
        diagnostics.push(interpolation_runtime_unavailable_diagnostic(
            sources, expression,
        ));
        return;
    }

    if interpolation_format_method(&Type::Str, expression.span, resolved, environment).is_none() {
        diagnostics.push(interpolation_runtime_unavailable_diagnostic(
            sources, expression,
        ));
        return;
    }

    for part in &expression.parts {
        let InterpolatedStringPart::Expression(part) = part else {
            continue;
        };
        let actual = expression_type(&part.expression, resolved, environment);
        if actual.is_unknown_or_unresolved() {
            continue;
        }

        if interpolation_format_method(&actual, part.expression_span, resolved, environment)
            .is_none()
        {
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

pub(super) fn interpolation_format_method(
    ty: &Type,
    span: crate::source::ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<TypecheckProtocolMethod> {
    let runtime = resolved.trusted_declarations.interpolation_runtime()?;
    let receiver = method_self_type_for_receiver(ty);
    super::protocol_methods::resolved_protocol_method(
        &receiver,
        &runtime.format_interface_canonical_name,
        &runtime.format_method_name,
        span,
        resolved,
        environment,
    )
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
