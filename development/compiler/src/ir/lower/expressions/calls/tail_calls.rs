use super::*;

pub(in crate::ir::lower::expressions) fn lower_direct_tail_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if primitive_trap_call(call, context) {
        return Ok(vec![Instruction::Trap]);
    }

    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls in tail return position",
        )]);
    };

    validate_tail_call_return_type(&target, &callee_name, context)?;

    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) =
        lower_call_arguments(call, &target, &callee_name, context, &mut temporaries)?;

    if fallible_success_tail_call_requires_normal_call(&target, context)
        || call
            .arguments
            .iter()
            .any(|argument| context.expression_contains_borrow(argument.span()))
        || arguments
            .iter()
            .any(tail_call_argument_requires_current_frame)
        || call_arguments_require_stack(&arguments, &callee_name)?
    {
        let Some(return_type) = context.call_return_type(&target).cloned() else {
            return Err(unsupported_non_tail_return_call_diagnostic(&callee_name));
        };
        instructions.push(lower_non_tail_return_call_instruction(
            return_type,
            target,
            arguments,
            &callee_name,
        )?);
        instructions.push(Instruction::Return);
        return Ok(instructions);
    }

    instructions.push(Instruction::TailCall { target, arguments });
    Ok(instructions)
}

fn tail_call_argument_requires_current_frame(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::Borrow(_)) || is_tail_call_stack_pointer_argument(argument)
}

fn fallible_success_tail_call_requires_normal_call(
    target: &CallTarget,
    context: &LoweringContext,
) -> bool {
    if !matches!(context.function_return_type(), Type::Fallible(_)) {
        return false;
    }
    matches!(context.call_return_type(target), Some(return_type) if return_type == context.return_type())
}

fn lower_non_tail_return_call_instruction(
    return_type: Type,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    callee_name: &str,
) -> Result<Instruction, Vec<Diagnostic>> {
    match &return_type {
        Type::I32 => Ok(Instruction::CallI32 {
            destination: I32Location::Return,
            target,
            arguments,
        }),
        Type::U8 => Ok(Instruction::CallU8 {
            destination: U8Location::Return,
            target,
            arguments,
        }),
        Type::Usize => Ok(Instruction::CallUsize {
            destination: UsizeLocation::Return,
            target,
            arguments,
        }),
        Type::Bool => Ok(Instruction::CallBool {
            destination: BoolLocation::Return,
            target,
            arguments,
        }),
        Type::Str => Ok(Instruction::CallStr {
            destination: StrLocation::Return,
            target,
            arguments,
        }),
        Type::Slice { .. } => Ok(Instruction::CallSlice {
            destination: SliceLocation::Return,
            target,
            arguments,
        }),
        Type::Aggregate { layout } => Ok(aggregate_call_instruction(
            &return_type,
            AggregateLocation::Return,
            target,
            arguments,
            *layout,
        )),
        Type::DirectAggregate { layout, .. } => Ok(aggregate_call_instruction(
            &return_type,
            AggregateLocation::DirectReturn,
            target,
            arguments,
            *layout,
        )),
        Type::Never
        | Type::Void
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. }
        | Type::Borrow { .. }
        | Type::Error => Err(unsupported_non_tail_return_call_diagnostic(callee_name)),
    }
}

fn unsupported_non_tail_return_call_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower return call to function `{callee_name}` without tail-call support for this return type"
        ),
    )]
}

pub(in crate::ir::lower::expressions) fn is_tail_call_stack_pointer_argument(
    argument: &ScalarArgument,
) -> bool {
    matches!(argument, ScalarArgument::AggregateIndirect(_))
}
