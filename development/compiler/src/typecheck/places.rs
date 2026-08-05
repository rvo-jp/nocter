use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::{Expr, IndexExpr, MemberExpr};
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
        Expr::Index(index) => index_expression_is_writable_place(index, resolved, environment),
        _ => false,
    }
}

pub(super) fn expression_is_established_place(expression: &Expr) -> bool {
    match unwrap_group(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => expression_is_established_place(&member.object),
        Expr::Index(index) => expression_is_established_place(&index.object),
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

fn index_expression_is_writable_place(
    index: &IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match expression_type(&index.object, resolved, environment) {
        Type::View {
            is_readwrite: true, ..
        } => true,
        Type::View {
            is_readwrite: false,
            ..
        }
        | Type::Str => false,
        Type::Array { .. } => expression_is_writable_place(&index.object, resolved, environment),
        _ => false,
    }
}

fn type_is_readwrite_borrow(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Borrow {
            is_readwrite: true,
            ..
        }
    )
}

fn type_is_readonly_borrow(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Borrow {
            is_readwrite: false,
            ..
        }
    )
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}
