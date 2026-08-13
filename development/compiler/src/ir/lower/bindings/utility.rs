use super::*;

pub(super) fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn expression_is_pointer_address_value(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Call(call) => matches!(
            context.intrinsic_for_call(call),
            Some(
                crate::intrinsics::IntrinsicId::FromAddr
                    | crate::intrinsics::IntrinsicId::FromRef
                    | crate::intrinsics::IntrinsicId::FromRefMut
            )
        ),
        Expr::Group(group) => expression_is_pointer_address_value(&group.expression, context),
        _ => false,
    }
}

pub(super) fn macos_syscall_primitive_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.intrinsic_for_call(call),
        Some(
            crate::intrinsics::IntrinsicId::Syscall(0)
                | crate::intrinsics::IntrinsicId::Syscall(1)
                | crate::intrinsics::IntrinsicId::Syscall(2)
                | crate::intrinsics::IntrinsicId::Syscall(3)
                | crate::intrinsics::IntrinsicId::Syscall(4)
                | crate::intrinsics::IntrinsicId::Syscall(5)
                | crate::intrinsics::IntrinsicId::Syscall(6)
        )
    )
}
