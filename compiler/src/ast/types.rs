use super::TypeExpr;
use std::collections::HashMap;

pub(crate) fn substitute_type_expr_parameters(
    ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> TypeExpr {
    match ty {
        TypeExpr::Reference(reference) => substitutions
            .get(&reference.name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        TypeExpr::Generic(generic) => {
            let mut generic = generic.clone();
            generic.arguments = generic
                .arguments
                .iter()
                .map(|argument| substitute_type_expr_parameters(argument, substitutions))
                .collect();
            TypeExpr::Generic(generic)
        }
        TypeExpr::Pointer(pointer) => {
            let mut pointer = pointer.clone();
            pointer.inner = Box::new(substitute_type_expr_parameters(
                &pointer.inner,
                substitutions,
            ));
            TypeExpr::Pointer(pointer)
        }
        TypeExpr::Borrow(borrow) => {
            let mut borrow = borrow.clone();
            borrow.inner = Box::new(substitute_type_expr_parameters(
                &borrow.inner,
                substitutions,
            ));
            TypeExpr::Borrow(borrow)
        }
        TypeExpr::View(view) => {
            let mut view = view.clone();
            view.element = Box::new(substitute_type_expr_parameters(
                &view.element,
                substitutions,
            ));
            TypeExpr::View(view)
        }
        TypeExpr::Array(array) => {
            let mut array = array.clone();
            array.element = Box::new(substitute_type_expr_parameters(
                &array.element,
                substitutions,
            ));
            TypeExpr::Array(array)
        }
        TypeExpr::Optional(optional) => {
            let mut optional = optional.clone();
            optional.inner = Box::new(substitute_type_expr_parameters(
                &optional.inner,
                substitutions,
            ));
            TypeExpr::Optional(optional)
        }
        TypeExpr::Fallible(fallible) => {
            let mut fallible = fallible.clone();
            fallible.success = Box::new(substitute_type_expr_parameters(
                &fallible.success,
                substitutions,
            ));
            fallible.error = Box::new(substitute_type_expr_parameters(
                &fallible.error,
                substitutions,
            ));
            TypeExpr::Fallible(fallible)
        }
    }
}
