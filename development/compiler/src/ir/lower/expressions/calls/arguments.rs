use super::*;
use crate::ir::lower::expressions::lower_integer_expression_to_value;

pub(in crate::ir::lower::expressions) fn lower_call_arguments(
    call: &CallExpr,
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    lower_call_arguments_with_explicit_types(call, target, callee_name, context, temporaries, None)
}

pub(in crate::ir::lower) fn lower_call_arguments_with_explicit_types(
    call: &CallExpr,
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    explicit_parameter_types: Option<&[TypeExpr]>,
) -> Result<(Vec<Instruction>, Vec<ScalarArgument>), Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!("IR cannot find the specialized signature for planned call `{callee_name}`"),
        )]);
    };
    let mut evaluation = CallEvaluationContext::new(context, temporaries)?;

    let method_receiver = context.method_call_receiver(call);
    let argument_count = call.arguments.len() + usize::from(method_receiver.is_some());
    if parameter_types.len() != argument_count {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "native lowering cannot lower call to function `{callee_name}` with {} arguments against {} parameters",
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
                context.call_argument_uses_implicit_readonly_borrow(call, index),
                explicit_parameter_types
                    .and_then(|types| types.get(index).cloned())
                    .or_else(|| context.call_argument_parameter_type_expr(call, index)),
            )
        }));
    for ((argument, is_method_receiver, parameter_type_expr), parameter_type) in
        call_arguments.zip(parameter_types)
    {
        evaluation.sync_temporaries(temporaries)?;
        let context = evaluation.context();
        match parameter_type {
            Type::I32 => {
                let argument = lower_i32_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    context,
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::I32(argument.value));
            }
            Type::U8 => {
                let argument = lower_u8_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    context,
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::U8(argument.value));
            }
            Type::Usize => {
                let argument = lower_usize_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    context,
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Usize(argument.value));
            }
            Type::Integer(kind) => {
                let argument = lower_integer_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    *kind,
                    context,
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Usize(argument.value));
            }
            Type::Bool => {
                let argument = lower_bool_expression_to_value_with_temporaries(
                    unwrap_copy_move_argument(argument),
                    context,
                    "E8006",
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Bool(argument.value));
            }
            Type::Str => {
                let argument = lower_str_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    context,
                    temporaries,
                )?;
                instructions.extend(argument.instructions);
                arguments.push(ScalarArgument::Str(argument.value));
            }
            Type::Slice { .. } => {
                let argument = lower_slice_expression_to_value(
                    unwrap_copy_move_argument(argument),
                    context,
                    temporaries,
                )?;
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
                let (argument_instructions, source) = if let Some(lowered) =
                    lower_tracked_closure_argument_source(
                        argument,
                        parameter_type,
                        parameter_type_expr.as_ref(),
                        callee_name,
                        &mut evaluation,
                        temporaries,
                    )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_spread_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_interpolation_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_payload_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_struct_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_array_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else {
                    lower_aggregate_argument_source(
                        argument,
                        is_method_receiver,
                        parameter_type,
                        parameter_type_expr.as_ref(),
                        callee_name,
                        evaluation.context(),
                        temporaries,
                    )?
                };
                retain_owned_aggregate_argument(
                    &mut evaluation,
                    parameter_type_expr.as_ref(),
                    source,
                    parameter_type,
                    callee_name,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateIndirect(AggregateArgument {
                    source,
                }));
            }
            Type::DirectAggregate { layout, words } => {
                let (argument_instructions, source) = if let Some(lowered) =
                    lower_tracked_closure_argument_source(
                        argument,
                        parameter_type,
                        parameter_type_expr.as_ref(),
                        callee_name,
                        &mut evaluation,
                        temporaries,
                    )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_spread_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_interpolation_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_payload_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_struct_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else if let Some(lowered) = lower_tracked_array_argument_source(
                    argument,
                    parameter_type,
                    parameter_type_expr.as_ref(),
                    callee_name,
                    &mut evaluation,
                    temporaries,
                )? {
                    lowered
                } else {
                    lower_aggregate_argument_source(
                        argument,
                        is_method_receiver,
                        parameter_type,
                        parameter_type_expr.as_ref(),
                        callee_name,
                        evaluation.context(),
                        temporaries,
                    )?
                };
                retain_owned_aggregate_argument(
                    &mut evaluation,
                    parameter_type_expr.as_ref(),
                    source,
                    parameter_type,
                    callee_name,
                )?;
                instructions.extend(argument_instructions);
                arguments.push(ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source,
                    layout: *layout,
                    words: *words,
                }));
            }
            Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. } => {
                arguments.push(lower_stored_outcome_argument(
                    argument,
                    parameter_type_expr.as_ref(),
                    context,
                )?);
            }
            Type::Void | Type::Never => {
                return Err(vec![Diagnostic::error(
                    "E8006",
                    format!(
                        "native lowering can only lower scalar call arguments for function `{callee_name}`, got `{}`",
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

fn retain_owned_aggregate_argument(
    evaluation: &mut CallEvaluationContext<'_, '_>,
    parameter_type_expr: Option<&TypeExpr>,
    source: AggregateArgumentSource,
    parameter_type: &Type,
    callee_name: &str,
) -> Result<(), Vec<Diagnostic>> {
    let slot_index = match source {
        AggregateArgumentSource::Slot(slot_index) => slot_index,
        AggregateArgumentSource::SlotField { .. } => return Ok(()),
    };
    if evaluation
        .context()
        .aggregate_local_by_slot(slot_index)
        .is_some()
    {
        return Ok(());
    }
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(());
    };
    let Some(drop_kind) = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr)
    else {
        return Ok(());
    };
    let layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(()),
    };
    if !evaluation.complete_temporary(slot_index, layout, drop_kind) {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!("cannot retain owned aggregate argument for function `{callee_name}`"),
        )]);
    }
    Ok(())
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
        format!("native lowering cannot lower error argument for function `{callee_name}`"),
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

fn unwrap_copy_move_argument(expression: &Expr) -> &Expr {
    match unwrap_group(expression) {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => &unary.operand,
        expression => expression,
    }
}

fn call_argument_abi_word_count_overflow_diagnostic(callee_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "native lowering call argument ABI word count overflows for function `{callee_name}`"
        ),
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
            "native lowering produced {actual} argument ABI words for function `{callee_name}`, but the resolved signature expects {expected}"
        ),
    )]
}
