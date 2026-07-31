use super::*;

pub(in crate::ir::lower::expressions) fn lower_call_arguments(
    call: &CallExpr,
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(target) else {
        return lower_legacy_i32_call_arguments(call, callee_name, context, temporaries);
    };

    let method_receiver = context.method_call_receiver(call);
    let argument_count = call.arguments.len() + usize::from(method_receiver.is_some());
    if parameter_types.len() != argument_count {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 cannot lower call to function `{callee_name}` with {} arguments against {} parameters",
                argument_count,
                parameter_types.len(),
            ),
        )]);
    }

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    let call_arguments = method_receiver
        .into_iter()
        .map(|receiver| (receiver, true, None))
        .chain(call.arguments.iter().enumerate().map(|(index, argument)| {
            (
                argument,
                false,
                context.call_argument_parameter_type_expr(call, index),
            )
        }));
    for ((argument, is_method_receiver, parameter_type_expr), parameter_type) in
        call_arguments.zip(parameter_types)
    {
        match parameter_type {
            Type::I32 => {
                let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::I32(argument.value));
            }
            Type::U8 => {
                let argument = lower_u8_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::U8(argument.value));
            }
            Type::Usize => {
                let argument = lower_usize_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Usize(argument.value));
            }
            Type::Bool => {
                let argument = lower_bool_expression_to_value_with_temporaries(
                    argument,
                    context,
                    "E8006",
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Bool(argument.value));
            }
            Type::Str => {
                let argument = lower_str_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Str(argument.value));
            }
            Type::Slice { .. } => {
                let argument = lower_slice_expression_to_value(argument, context, temporaries)?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Slice(argument.value));
            }
            Type::Error => {
                let (argument_instructions, code, message) =
                    lower_error_argument(argument, callee_name, context, temporaries)?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::Str(code));
                arguments.push(ScalarArgument::Str(message));
            }
            Type::Borrow { .. } => {
                let (argument_instructions, argument) = if is_method_receiver {
                    lower_implicit_receiver_borrow_argument(
                        argument,
                        parameter_type,
                        callee_name,
                        context,
                        temporaries,
                    )?
                } else {
                    lower_borrow_argument(
                        argument,
                        parameter_type,
                        callee_name,
                        context,
                        temporaries,
                    )?
                };
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::Borrow(argument));
            }
            Type::Aggregate { .. } => {
                let (argument_instructions, source) = lower_aggregate_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    context,
                    temporaries,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateIndirect(AggregateArgument {
                    source,
                }));
            }
            Type::DirectAggregate { layout, words } => {
                let (argument_instructions, source) = lower_aggregate_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    context,
                    temporaries,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source,
                    layout: *layout,
                    words: *words,
                }));
            }
            Type::Void | Type::Never | Type::Fallible(_) => {
                return Err(vec![Diagnostic::error(
                    "E8006",
                    format!(
                        "IR v0 can only lower scalar call arguments for function `{callee_name}`, got `{}`",
                        describe_type(parameter_type),
                    ),
                )]);
            }
        }
    }

    let actual_abi_words = validate_call_argument_abi_word_count(callee_name, &arguments)?;
    if let Some(expected_abi_words) = context.call_parameter_abi_word_count(target)
        && actual_abi_words != expected_abi_words
    {
        return Err(call_argument_abi_word_count_mismatch_diagnostic(
            callee_name,
            expected_abi_words,
            actual_abi_words,
        ));
    }
    Ok((instructions, arguments))
}

fn lower_error_argument(
    argument: &Expr,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, StrValue, StrValue), Vec<Diagnostic>> {
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_error_argument_diagnostic(callee_name));
    };

    let code = temporaries.next_str()?;
    let message = temporaries.next_str()?;
    let payload_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    let Some(payload) =
        lower_error_payload(argument, resolved, root_source, Some(&payload_context))?
    else {
        return Err(unsupported_error_argument_diagnostic(callee_name));
    };

    Ok((
        payload.into_store_instructions(code, message),
        StrValue::Location(code),
        StrValue::Location(message),
    ))
}

fn unsupported_error_argument_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("IR v0 cannot lower error argument for function `{callee_name}`"),
    )]
}

fn validate_call_argument_abi_word_count(
    callee_name: &str,
    arguments: &[ScalarArgument],
) -> Result<usize, Vec<Diagnostic>> {
    call_argument_abi_word_count(arguments, callee_name)
}

pub(in crate::ir::lower::expressions) fn call_arguments_require_stack(
    arguments: &[ScalarArgument],
    callee_name: &str,
) -> Result<bool, Vec<Diagnostic>> {
    Ok(call_argument_abi_word_count(arguments, callee_name)? > ARGUMENT_REGISTER_COUNT)
}

fn call_argument_abi_word_count(
    arguments: &[ScalarArgument],
    callee_name: &str,
) -> Result<usize, Vec<Diagnostic>> {
    let mut count = 0_usize;
    for argument in arguments {
        count = count
            .checked_add(argument.abi_word_count())
            .ok_or_else(|| call_argument_abi_word_count_overflow_diagnostic(callee_name))?;
    }

    Ok(count)
}

fn call_argument_abi_word_count_overflow_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("IR v0 call argument ABI word count overflows for function `{callee_name}`"),
    )]
}

fn call_argument_abi_word_count_mismatch_diagnostic(
    callee_name: &str,
    expected: usize,
    actual: usize,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 lowered call arguments for function `{callee_name}` into {actual} ABI words, but the resolved signature expects {expected}"
        ),
    )]
}

fn lower_legacy_i32_call_arguments(
    call: &CallExpr,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for argument in &call.arguments {
        let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
        instructions.extend(argument.instructions);
        arguments.push(ScalarArgument::I32(argument.value));
    }

    let _actual_abi_words = validate_call_argument_abi_word_count(callee_name, &arguments)?;
    Ok((instructions, arguments))
}
