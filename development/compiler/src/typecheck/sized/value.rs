use crate::ast::TypeExpr;
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::SourceMap;

use super::super::diagnostics::unsized_value_type_diagnostic;
use super::super::model::Type;
use super::super::type_expr::type_expr_to_type_with_self_type;

pub(in crate::typecheck::sized) fn check_value_type(
    sources: &SourceMap,
    ty: &TypeExpr,
    subject: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(unsized_part) = first_unsized_value_part(ty, resolved, self_type) {
        diagnostics.push(unsized_value_type_diagnostic(
            sources,
            ty,
            subject,
            &unsized_part,
        ));
    }
}

fn first_unsized_value_part(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    if let Some(interface_type) = interface_type_part(ty, resolved, self_type) {
        return Some(interface_type);
    }

    let resolved_type = type_expr_to_type_with_self_type(ty, resolved, self_type);
    resolved_type
        .first_unsized_part()
        .cloned()
        .or_else(|| first_unsized_generic_argument(ty, resolved, self_type))
}

fn interface_type_part(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    let resolved_type = type_expr_to_type_with_self_type(ty, resolved, self_type);
    if type_is_interface(&resolved_type, resolved) {
        Some(resolved_type)
    } else {
        None
    }
}

fn type_is_interface(ty: &Type, resolved: &ResolveOutput) -> bool {
    match ty {
        Type::Named(name) | Type::Generic { name, .. } => resolved
            .type_symbol_by_canonical_name(name)
            .is_some_and(|symbol| symbol.kind == TypeSymbolKind::Interface),
        _ => false,
    }
}

fn first_unsized_generic_argument(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    match ty {
        TypeExpr::Callable(_) => Some(type_expr_to_type_with_self_type(ty, resolved, self_type)),
        TypeExpr::Closure(closure) => closure
            .captures
            .iter()
            .find_map(|capture| first_unsized_value_part(&capture.ty, resolved, self_type)),
        TypeExpr::Opaque(opaque) => opaque
            .witness
            .as_ref()
            .and_then(|witness| first_unsized_value_part(witness, resolved, self_type)),
        TypeExpr::Reference(_) => interface_type_part(ty, resolved, self_type),
        TypeExpr::Generic(generic) => interface_type_part(ty, resolved, self_type).or_else(|| {
            generic
                .arguments
                .iter()
                .find_map(|argument| first_unsized_value_part(argument, resolved, self_type))
        }),
        TypeExpr::Projection(_) => {
            let projected = type_expr_to_type_with_self_type(ty, resolved, self_type);
            projected.first_unsized_part().cloned()
        }
        TypeExpr::Array(array) => first_unsized_value_part(&array.element, resolved, self_type),
        TypeExpr::Pointer(pointer) => {
            first_unsized_generic_argument(&pointer.inner, resolved, self_type)
        }
        TypeExpr::Borrow(borrow) => {
            first_unsized_generic_argument(&borrow.inner, resolved, self_type)
        }
        TypeExpr::View(view) => first_unsized_generic_argument(&view.element, resolved, self_type),
        TypeExpr::Optional(optional) => {
            first_unsized_value_part(&optional.inner, resolved, self_type)
        }
        TypeExpr::Fallible(fallible) => {
            first_unsized_value_part(&fallible.success, resolved, self_type)
                .or_else(|| first_unsized_value_part(&fallible.error, resolved, self_type))
        }
    }
}
