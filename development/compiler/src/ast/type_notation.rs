use super::TypeExpr;
use crate::type_notation::{PostfixOperator, PrefixOperator, TypeNotation, TypeNotationParameter};

pub(crate) fn type_expr_notation(ty: &TypeExpr) -> TypeNotation {
    match ty {
        TypeExpr::Callable(callable) => TypeNotation::Callable {
            capability_prefix: callable.capability.source_prefix(),
            parameters: callable
                .parameters
                .iter()
                .map(|parameter| TypeNotationParameter {
                    name: parameter.name.clone(),
                    ty: type_expr_notation(&parameter.ty),
                })
                .collect(),
            return_type: Box::new(type_expr_notation(&callable.return_type)),
            provenance: callable
                .result_provenance
                .iter()
                .flat_map(|clause| clause.origins.iter())
                .map(|origin| origin.kind.source_label().to_string())
                .collect(),
        },
        TypeExpr::Closure(closure) => TypeNotation::Atom(closure.identity_name()),
        TypeExpr::Opaque(opaque) => {
            let (interface_name, interface_arguments) = match opaque.interface.as_ref() {
                TypeExpr::Reference(reference) => (reference.name.clone(), Vec::new()),
                TypeExpr::Generic(generic) => (
                    generic.name.clone(),
                    generic.arguments.iter().map(type_expr_notation).collect(),
                ),
                other => (canonical_type_expr(other), Vec::new()),
            };
            TypeNotation::Opaque {
                interface_name,
                interface_arguments,
                associated_bindings: opaque
                    .associated_bindings
                    .iter()
                    .map(|binding| (binding.name.clone(), type_expr_notation(&binding.value)))
                    .collect(),
            }
        }
        TypeExpr::Reference(reference) => TypeNotation::Atom(reference.name.clone()),
        TypeExpr::Generic(generic) => TypeNotation::Generic {
            name: generic.name.clone(),
            arguments: generic.arguments.iter().map(type_expr_notation).collect(),
        },
        TypeExpr::Projection(projection) => TypeNotation::Projection {
            base: Box::new(type_expr_notation(&projection.base)),
            member: projection.name.clone(),
        },
        TypeExpr::Pointer(pointer) => prefix(PrefixOperator::Pointer, &pointer.inner),
        TypeExpr::Borrow(borrow) => prefix(
            if borrow.is_readwrite {
                PrefixOperator::ReadwriteBorrow
            } else {
                PrefixOperator::ReadonlyBorrow
            },
            &borrow.inner,
        ),
        TypeExpr::View(view) if view.is_readwrite => TypeNotation::Prefix {
            operator: PrefixOperator::ReadwriteBorrow,
            inner: Box::new(TypeNotation::View(Box::new(type_expr_notation(
                &view.element,
            )))),
        },
        TypeExpr::View(view) => TypeNotation::View(Box::new(type_expr_notation(&view.element))),
        TypeExpr::Array(array) => TypeNotation::Array {
            element: Box::new(type_expr_notation(&array.element)),
            length: array.length.value.clone(),
        },
        TypeExpr::Optional(optional) => postfix(PostfixOperator::Optional, &optional.inner),
        TypeExpr::Fallible(fallible) => postfix(PostfixOperator::Fallible, &fallible.success),
    }
}

pub(crate) fn canonical_type_expr(ty: &TypeExpr) -> String {
    type_expr_notation(ty).render()
}

fn prefix(operator: PrefixOperator, inner: &TypeExpr) -> TypeNotation {
    TypeNotation::Prefix {
        operator,
        inner: Box::new(type_expr_notation(inner)),
    }
}

fn postfix(operator: PostfixOperator, inner: &TypeExpr) -> TypeNotation {
    TypeNotation::Postfix {
        inner: Box::new(type_expr_notation(inner)),
        operator,
    }
}
