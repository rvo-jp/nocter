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
            context.primitive_name_for_call(call),
            Some("from_addr" | "from_ref" | "from_ref_mut")
        ),
        Expr::Group(group) => expression_is_pointer_address_value(&group.expression, context),
        _ => false,
    }
}

pub(super) fn macos_syscall_primitive_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}
