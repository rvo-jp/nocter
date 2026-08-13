use super::*;

pub(in crate::ir::lower::functions) fn aggregate_return_layout_and_destination(
    return_type: &Type,
) -> (crate::abi::ValueLayout, AggregateLocation) {
    match return_type {
        Type::Aggregate { layout } => (*layout, AggregateLocation::Return),
        Type::DirectAggregate { layout, .. } => (*layout, AggregateLocation::DirectReturn),
        _ => unreachable!("aggregate return lowering requires aggregate return type"),
    }
}

pub(in crate::ir::lower::functions) fn validate_aggregate_call_success_return_passing(
    target: &CallTarget,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = context.call_success_return_passing(target) else {
        return Ok(());
    };
    let Some(expected) = return_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }

    Err(aggregate_call_return_abi_mismatch_diagnostic(
        function_name,
        expected,
        actual,
    ))
}

pub(in crate::ir::lower::functions) fn aggregate_call_return_abi_mismatch_diagnostic(
    function_name: &str,
    expected: crate::abi::ReturnPassing,
    actual: crate::abi::ReturnPassing,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "native lowering aggregate return ABI mismatch in function `{function_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ir::lower::functions) enum AggregateValueUse {
    ImplicitCopy,
    ExplicitMove,
}

pub(in crate::ir::lower::functions) fn unsupported_aggregate_return_diagnostic(
    function_name: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "native lowering can only lower aggregate returns from function `{function_name}` from a supported struct literal, an aggregate call, or a supported aggregate local slot"
        ),
    )]
}

pub(in crate::ir::lower::functions) fn macos_syscall_primitive_call(
    call: &crate::ast::CallExpr,
    context: &LoweringContext,
) -> bool {
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
