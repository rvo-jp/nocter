use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::{BindingKind, Expr};
use crate::resolve::ResolveOutput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptionalBorrowProjection {
    pub(super) is_readwrite: bool,
    pub(super) projected_type: Type,
}

pub(super) fn optional_borrow_projection_type(
    initializer: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<OptionalBorrowProjection> {
    let Expr::Borrow(borrow) = unwrap_group(initializer) else {
        return None;
    };
    let Type::Optional(inner) = expression_type(&borrow.expression, resolved, environment) else {
        return None;
    };
    if inner.is_unknown_or_unresolved() {
        return Some(OptionalBorrowProjection {
            is_readwrite: borrow.is_readwrite,
            projected_type: Type::Unknown,
        });
    }

    Some(OptionalBorrowProjection {
        is_readwrite: borrow.is_readwrite,
        projected_type: borrowed_projection_type(*inner, borrow.is_readwrite),
    })
}

pub(super) fn optional_projection_binding_kind_is_allowed(
    kind: BindingKind,
    is_readwrite: bool,
) -> bool {
    matches!(
        (kind, is_readwrite),
        (BindingKind::Let, false) | (BindingKind::Var, true)
    )
}

fn borrowed_projection_type(inner: Type, is_readwrite: bool) -> Type {
    match (is_readwrite, inner) {
        (false, Type::StrData) => Type::Str,
        (_, Type::ArrayData { element }) => Type::View {
            is_readwrite,
            element,
        },
        (_, inner) => Type::Named(format!(
            "{}{}",
            if is_readwrite { "&+" } else { "&" },
            inner.display()
        )),
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}
