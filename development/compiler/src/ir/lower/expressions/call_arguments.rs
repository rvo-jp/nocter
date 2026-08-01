use super::*;

pub(in crate::ir::lower) fn lower_call_arguments_to_scalar_arguments(
    call: &CallExpr,
    target: &crate::ir::CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_call_arguments(call, target, callee_name, context, &mut temporaries)
}

pub(in crate::ir::lower) fn lower_call_arguments_to_scalar_arguments_with_temporaries(
    call: &CallExpr,
    target: &crate::ir::CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    lower_call_arguments(call, target, callee_name, context, temporaries)
}
