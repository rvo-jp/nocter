use crate::ast::TypeExpr;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
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
    let resolved_type = type_expr_to_type_with_self_type(ty, resolved, self_type);
    resolved_type
        .first_unsized_part()
        .cloned()
        .or_else(|| first_unsized_generic_argument(ty, resolved, self_type))
}

fn first_unsized_generic_argument(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Type> {
    match ty {
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .find_map(|argument| first_unsized_value_part(argument, resolved, self_type)),
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
        TypeExpr::Reference(_) => None,
    }
}
