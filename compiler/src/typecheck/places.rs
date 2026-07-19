use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::{Expr, MemberExpr};
use crate::resolve::ResolveOutput;

pub(super) fn expression_is_writable_place(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => {
            environment.get(&identifier.name).is_some()
                && environment.is_mutable_binding(&identifier.name)
        }
        Expr::Member(member) => field_member_is_writable_place(member, resolved, environment),
        _ => false,
    }
}

pub(super) fn field_member_is_writable_place(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let object_type = expression_type(&member.object, resolved, environment);
    if type_is_readwrite_borrow(&object_type) {
        return true;
    }
    if type_is_readonly_borrow(&object_type) {
        return false;
    }

    expression_is_writable_place(&member.object, resolved, environment)
}

fn type_is_readwrite_borrow(ty: &Type) -> bool {
    matches!(ty, Type::Named(name) if name.starts_with("&+"))
}

fn type_is_readonly_borrow(ty: &Type) -> bool {
    matches!(ty, Type::Named(name) if name.starts_with('&') && !name.starts_with("&+"))
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}
