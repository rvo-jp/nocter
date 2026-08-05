use super::TypeExpr;
use std::collections::HashMap;

pub(crate) fn type_expr_display_lossy(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Callable(callable) => {
            let parameters = callable
                .parameters
                .iter()
                .map(|parameter| {
                    let ty = type_expr_display_lossy(&parameter.ty);
                    parameter
                        .name
                        .as_ref()
                        .map_or(ty.clone(), |name| format!("{name}: {ty}"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let provenance =
                callable
                    .result_provenance
                    .as_ref()
                    .map_or_else(String::new, |clause| {
                        format!(
                            " from {}",
                            clause
                                .origins
                                .iter()
                                .map(|origin| origin.kind.source_label())
                                .collect::<Vec<_>>()
                                .join(" | ")
                        )
                    });
            format!(
                "{}func({parameters}): {}{provenance}",
                callable.capability.source_prefix(),
                type_expr_display_lossy(&callable.return_type)
            )
        }
        TypeExpr::Closure(closure) => closure.identity_name(),
        TypeExpr::Reference(reference) => reference.name.clone(),
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(type_expr_display_lossy)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", generic.name)
        }
        TypeExpr::Pointer(pointer) => format!("*{}", type_expr_display_lossy(&pointer.inner)),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            format!("&+{}", type_expr_display_lossy(&borrow.inner))
        }
        TypeExpr::Borrow(borrow) => format!("&{}", type_expr_display_lossy(&borrow.inner)),
        TypeExpr::View(view) if view.is_readwrite => {
            format!("&+[{}]", type_expr_display_lossy(&view.element))
        }
        TypeExpr::View(view) => format!("[{}]", type_expr_display_lossy(&view.element)),
        TypeExpr::Array(array) => {
            format!(
                "[{}; {}]",
                type_expr_display_lossy(&array.element),
                array.length.value
            )
        }
        TypeExpr::Optional(optional) => format!("{}?", type_expr_display_lossy(&optional.inner)),
        TypeExpr::Fallible(fallible) => format!("{}!", type_expr_display_lossy(&fallible.success)),
    }
}

pub(crate) fn substitute_type_expr_parameters(
    ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> TypeExpr {
    match ty {
        TypeExpr::Callable(callable) => {
            let mut callable = callable.clone();
            for parameter in &mut callable.parameters {
                parameter.ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
            }
            callable.return_type = Box::new(substitute_type_expr_parameters(
                &callable.return_type,
                substitutions,
            ));
            TypeExpr::Callable(callable)
        }
        TypeExpr::Closure(closure) => {
            let mut closure = closure.clone();
            for capture in &mut closure.captures {
                capture.ty = substitute_type_expr_parameters(&capture.ty, substitutions);
            }
            closure.parameters = closure
                .parameters
                .iter()
                .map(|parameter| substitute_type_expr_parameters(parameter, substitutions))
                .collect();
            closure.return_type = Box::new(substitute_type_expr_parameters(
                &closure.return_type,
                substitutions,
            ));
            TypeExpr::Closure(closure)
        }
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
