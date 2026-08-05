use super::*;

pub(super) fn lower_aggregate_struct_literal_statement(
    literal: &crate::ast::StructLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_aggregate_literal_statement_diagnostic());
    };
    let value = abi_value_from_type_expr(&literal.ty, resolved)
        .map_err(|_error| unsupported_aggregate_literal_statement_diagnostic())?;
    if !supported_aggregate_copy_layout(value.layout) {
        return Err(unsupported_aggregate_literal_statement_diagnostic());
    }

    let drop_kind = context.aggregate_drop_for_type_expr(&literal.ty);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        value.layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        "expression statements",
        resolved,
        context,
        &mut temporaries,
    )?);
    append_discarded_aggregate_drop(
        &mut instructions,
        drop_kind,
        value.layout,
        slot_index,
        context,
    )?;
    Ok(instructions)
}

pub(super) fn lower_fallible_void_expression_statement(
    expression: &Expr,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_write_text_raw_call(call, context)
                || primitive_write_bytes_raw_call(call, context)
            {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_fallible_void_normal_call(
                    call,
                    context,
                    &mut temporaries,
                    failure_mode,
                )
                .map(Some);
            }

            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            let Some((_, success_type)) = context
                .call_return_type(&target)
                .and_then(Type::single_outcome)
            else {
                return Ok(None);
            };

            let mut temporaries = TemporaryAllocator::new(context)?;
            match success_type {
                Type::Void => {
                    lower_fallible_void_normal_call(call, context, &mut temporaries, failure_mode)
                }
                Type::I32 => {
                    let destination = temporaries.next_i32()?;
                    lower_fallible_i32_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::U8 => {
                    let destination = temporaries.next_u8()?;
                    lower_fallible_u8_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Usize => {
                    let destination = temporaries.next_usize()?;
                    lower_fallible_usize_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Bool => {
                    let destination = temporaries.next_bool()?;
                    lower_fallible_bool_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Str => {
                    let destination = temporaries.next_str()?;
                    lower_fallible_str_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Slice { .. } => {
                    let destination = temporaries.next_slice()?;
                    lower_fallible_slice_normal_call(
                        call,
                        destination,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
                    lower_aggregate_fallible_call_statement(
                        call,
                        success_type,
                        context,
                        &mut temporaries,
                        failure_mode,
                    )
                }
                _ => return Ok(None),
            }
            .map(Some)
        }
        Expr::Group(group) => {
            lower_fallible_void_expression_statement(&group.expression, context, failure_mode)
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_aggregate_normal_call_statement(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_type_expr) = context.call_value_type_expr(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let drop_kind = context.aggregate_drop_for_type_expr(&return_type_expr);
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    if macos_syscall_primitive_call(call, context) {
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            temporaries,
        )?
        else {
            return Err(unsupported_aggregate_call_statement_diagnostic());
        };
        instructions.append(&mut syscall_instructions);
    } else {
        let (mut argument_instructions, arguments) =
            lower_call_arguments(call, &target, &call_name, context, temporaries)?;
        instructions.append(&mut argument_instructions);
        push_aggregate_call_instruction(
            &mut instructions,
            &return_type,
            AggregateLocation::Slot(slot_index),
            target,
            arguments,
            layout,
        );
    }
    append_discarded_aggregate_drop(&mut instructions, drop_kind, layout, slot_index, context)?;
    Ok(instructions)
}

pub(super) fn lower_aggregate_fallible_call_statement(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_type_expr) = context.call_value_type_expr(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let drop_kind = context.aggregate_drop_for_type_expr(&return_type_expr);
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    let Some(layout) = aggregate_type_layout(success_type) else {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    };
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_call_statement_diagnostic());
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    append_discarded_aggregate_drop(&mut instructions, drop_kind, layout, slot_index, context)?;
    Ok(instructions)
}

pub(super) fn append_discarded_aggregate_drop(
    instructions: &mut Vec<Instruction>,
    drop_kind: Option<AggregateDrop>,
    layout: ValueLayout,
    slot_index: usize,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(drop_kind) = drop_kind else {
        return Ok(());
    };
    match &drop_kind {
        AggregateDrop::Direct(drop_glue) => {
            let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
                return Err(unsupported_aggregate_call_statement_diagnostic());
            };
            if parameter_types.len() != 1
                || !drop_parameter_matches_aggregate_slot(&parameter_types[0], layout)
            {
                return Err(unsupported_aggregate_call_statement_diagnostic());
            }
        }
        AggregateDrop::Struct(_)
        | AggregateDrop::Array(_)
        | AggregateDrop::PayloadEnum(_)
        | AggregateDrop::Outcome(_) => {}
    }
    instructions.extend(
        lower_aggregate_drop_instructions(
            "discarded aggregate",
            slot_index,
            layout,
            &drop_kind,
            context,
        )
        .map_err(|_| unsupported_aggregate_call_statement_diagnostic())?,
    );
    Ok(())
}

pub(super) fn drop_parameter_matches_aggregate_slot(
    parameter_type: &Type,
    layout: ValueLayout,
) -> bool {
    let Type::Borrow {
        is_readwrite: true,
        inner,
    } = parameter_type
    else {
        return false;
    };

    match inner.as_ref() {
        Type::Aggregate {
            layout: parameter_layout,
        }
        | Type::DirectAggregate {
            layout: parameter_layout,
            ..
        } => *parameter_layout == layout,
        _ => false,
    }
}

pub(super) fn discarded_fallible_statement_reserved_abi_words(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<usize> {
    match expression {
        Expr::Call(call) if primitive_write_text_raw_call(call, context) => Some(0),
        Expr::Call(call) if primitive_write_bytes_raw_call(call, context) => Some(0),
        Expr::Call(call) => {
            let (target, _call_name) = context.direct_call_target_and_name(call)?;
            let (_, success_type) = context.call_return_type(&target)?.single_outcome()?;
            discarded_fallible_success_reserved_abi_words(success_type)
        }
        Expr::Group(group) => {
            discarded_fallible_statement_reserved_abi_words(&group.expression, context)
        }
        _ => None,
    }
}

pub(super) fn discarded_fallible_success_reserved_abi_words(success_type: &Type) -> Option<usize> {
    match success_type {
        Type::Void => Some(0),
        Type::I32 | Type::U8 | Type::Usize | Type::Bool => Some(1),
        Type::Str | Type::Slice { .. } => Some(2),
        _ => None,
    }
}
