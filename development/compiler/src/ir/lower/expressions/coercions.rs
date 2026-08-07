use super::*;
use crate::ir::CallTarget;

pub(super) fn lower_borrow_coercion_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let mut temporaries = match TemporaryAllocator::new(context) {
        Ok(temporaries) => temporaries,
        Err(diagnostics) => return Some(Err(diagnostics)),
    };
    lower_borrow_coercion_to_location_with_temporaries(
        expression,
        destination,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower) fn lower_borrow_coercion_to_location_with_temporaries(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let lowered = lower_coercion_receiver(expression, context, temporaries)?;
    Some(lowered.map(|(mut instructions, target, arguments)| {
        instructions.push(Instruction::CallBorrow {
            destination,
            target,
            arguments,
        });
        instructions
    }))
}

pub(super) fn lower_str_coercion_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let lowered = lower_coercion_receiver(expression, context, temporaries)?;
    Some(lowered.map(|(mut instructions, target, arguments)| {
        instructions.push(Instruction::CallStr {
            destination,
            target,
            arguments,
        });
        instructions
    }))
}

pub(super) fn lower_slice_coercion_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let lowered = lower_coercion_receiver(expression, context, temporaries)?;
    Some(lowered.map(|(mut instructions, target, arguments)| {
        instructions.push(Instruction::CallSlice {
            destination,
            target,
            arguments,
        });
        instructions
    }))
}

type LoweredCoercionReceiver = (Vec<Instruction>, CallTarget, Vec<ScalarArgument>);

fn lower_coercion_receiver(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<LoweredCoercionReceiver, Vec<Diagnostic>>> {
    let plan = context.coercion_plan(expression.span())?;
    let Some(inner) = context.ir_type_for_type_expr(&plan.self_ty) else {
        return Some(Err(unsupported_borrow_expression_diagnostic()));
    };
    let receiver_is_readwrite =
        plan.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow;
    let receiver_type = Type::Borrow {
        is_readwrite: receiver_is_readwrite,
        inner: Box::new(inner.clone()),
    };
    let Some(target) = context.coercion_call_target(&plan) else {
        return Some(Err(unavailable_call_target_diagnostic()));
    };
    let source_expression = match unwrap_group(expression) {
        Expr::TypeConversion(conversion) => unwrap_group(&conversion.expression),
        expression => expression,
    };
    let source_expression = match source_expression {
        Expr::Borrow(borrow) => &borrow.expression,
        expression => expression,
    };
    Some(
        lower_borrow_source_from_expression_without_coercion(
            source_expression,
            &inner,
            receiver_is_readwrite,
            &receiver_type,
            &plan.target_name,
            context,
            temporaries,
        )
        .map(|(instructions, source)| {
            (
                instructions,
                target,
                vec![ScalarArgument::Borrow(crate::ir::BorrowArgument { source })],
            )
        }),
    )
}
