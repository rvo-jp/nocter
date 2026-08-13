//! Exhaustive traversal of one type-expression tree.

use super::TypeExpr;

/// Visits `ty` and every nested type expression in source order.
pub(crate) fn visit_type_exprs<'a>(ty: &'a TypeExpr, visitor: &mut impl FnMut(&'a TypeExpr)) {
    visitor(ty);
    match ty {
        TypeExpr::Callable(callable) => {
            for parameter in &callable.parameters {
                visit_type_exprs(&parameter.ty, visitor);
            }
            visit_type_exprs(&callable.return_type, visitor);
        }
        TypeExpr::Closure(closure) => {
            for capture in &closure.captures {
                visit_type_exprs(&capture.ty, visitor);
            }
            for parameter in &closure.parameters {
                visit_type_exprs(parameter, visitor);
            }
            visit_type_exprs(&closure.return_type, visitor);
        }
        TypeExpr::Opaque(opaque) => {
            visit_type_exprs(&opaque.interface, visitor);
            for binding in &opaque.associated_bindings {
                visit_type_exprs(&binding.value, visitor);
            }
            if let Some(witness) = &opaque.witness {
                visit_type_exprs(witness, visitor);
            }
        }
        TypeExpr::Generic(generic) => {
            for argument in &generic.arguments {
                visit_type_exprs(argument, visitor);
            }
        }
        TypeExpr::Projection(projection) => visit_type_exprs(&projection.base, visitor),
        TypeExpr::Pointer(pointer) => visit_type_exprs(&pointer.inner, visitor),
        TypeExpr::Borrow(borrow) => visit_type_exprs(&borrow.inner, visitor),
        TypeExpr::View(view) => visit_type_exprs(&view.element, visitor),
        TypeExpr::Array(array) => visit_type_exprs(&array.element, visitor),
        TypeExpr::Optional(optional) => visit_type_exprs(&optional.inner, visitor),
        TypeExpr::Fallible(fallible) => {
            visit_type_exprs(&fallible.success, visitor);
            visit_type_exprs(&fallible.error, visitor);
        }
        TypeExpr::Reference(_) => {}
    }
}
