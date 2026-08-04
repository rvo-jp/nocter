use super::*;

pub(in crate::ir::lower) fn lower_terminal_return_statement_with_scope_drops(
    statement: &ReturnStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(expression) = &statement.expression
        && let Some(instructions) = lower_terminal_control_return_expression(
            expression,
            context,
            diagnostic_code,
            subject,
            sources,
        )?
    {
        return Ok(instructions);
    }

    lower_return_statement_with_scope_drops(statement, context, diagnostic_code)
}

pub(in crate::ir::lower) fn lower_return_statement_with_scope_drops(
    statement: &ReturnStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_type = context.function_return_type().clone();
    let success_type = return_type.success_type().clone();
    let function_name = context.function_name().to_string();

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) =
            lower_never_expression_with_scope_drops(expression, context)?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) = lower_stored_outcome_return(expression, context)?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && matches!(
            return_type,
            Type::Fallible(_) | Type::ComposedOutcome { .. }
        )
        && let Some((root_source, resolved)) = context.resolved_calls()
        && let Some(payload) =
            lower_error_payload(expression, resolved, root_source, Some(context))?
    {
        return append_scope_end_drops_before_exit(lower_fallible_failure(payload), context);
    }

    if let Some(expression) = &statement.expression
        && context.function_returns_optional()
        && expression_is_none_literal(expression)
    {
        return append_scope_end_drops_before_exit(vec![Instruction::ReturnOptionalNone], context);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) = lower_otherwise_scalar_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            context,
            diagnostic_code,
        )?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) = lower_otherwise_aggregate_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            context,
        )?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) =
            lower_value_return_with_scope_drops(&success_type, expression, &return_type, context)?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && matches!(success_type, Type::DirectAggregate { .. })
        && !context.pending_aggregate_drops().is_empty()
    {
        let Some((_root_source, resolved)) = context.resolved_calls() else {
            return Err(unsupported_return_diagnostic(
                diagnostic_code,
                &function_name,
                "aggregate",
            ));
        };
        return lower_direct_aggregate_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            &function_name,
            resolved,
            context,
        );
    }

    let return_instructions = match (&success_type, &statement.expression) {
        (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
        (Type::U8, Some(expression)) => lower_u8_return_expression(expression, context),
        (Type::Usize, Some(expression)) => lower_usize_return_expression(expression, context),
        (Type::Bool, Some(expression)) => {
            lower_bool_return_expression(expression, context, diagnostic_code)
        }
        (Type::Str, Some(expression)) => lower_str_return_expression(expression, context),
        (Type::Slice { .. }, Some(expression)) => {
            lower_slice_return_expression(expression, context)
        }
        (Type::Borrow { .. }, Some(expression)) => {
            let mut instructions = lower_borrow_expression_to_location(
                expression,
                UsizeLocation::Return,
                &success_type,
                context,
            )?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
        (Type::Aggregate { .. } | Type::DirectAggregate { .. }, Some(expression)) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_return_diagnostic(
                    diagnostic_code,
                    &function_name,
                    "aggregate",
                ));
            };
            lower_aggregate_return_expression(
                expression,
                &success_type,
                &function_name,
                resolved,
                context,
            )
        }
        (Type::Never, Some(_)) => Err(vec![Diagnostic::error(
            diagnostic_code,
            format!(
                "IR v0 can only lower never function `{function_name}` returns from `never` calls"
            ),
        )]),
        (Type::Void, None) => Ok(vec![Instruction::Return]),
        (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
            diagnostic_code,
            format!("IR v0 cannot lower value returns from void function `{function_name}`"),
        )]),
        (Type::I32, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "i32",
        )),
        (Type::U8, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "u8",
        )),
        (Type::Usize, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "usize",
        )),
        (Type::Bool, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "bool",
        )),
        (Type::Str, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "&str",
        )),
        (Type::Slice { .. }, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "slice",
        )),
        (Type::Aggregate { .. } | Type::DirectAggregate { .. }, None) => Err(
            unsupported_bare_return_diagnostic(diagnostic_code, &function_name, "aggregate"),
        ),
        (Type::Error, _) => Err(unsupported_return_diagnostic(
            diagnostic_code,
            &function_name,
            "error",
        )),
        (Type::Borrow { .. }, None) => Err(unsupported_return_diagnostic(
            diagnostic_code,
            &function_name,
            "borrow",
        )),
        (Type::Never, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "never",
        )),
        (Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. }, _) => Err(
            unsupported_return_diagnostic(diagnostic_code, &function_name, "nested fallible"),
        ),
    }?;

    let return_instructions = mark_outcome_success_returns(&return_type, return_instructions);
    append_scope_end_drops_before_exit(return_instructions, context)
}

pub(in crate::ir::lower) fn lower_direct_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(success_type);
    if !matches!(destination, AggregateLocation::DirectReturn)
        || !supported_aggregate_copy_layout(expected_layout)
    {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_return_expression_to_location(
        expression,
        success_type,
        AggregateLocation::Slot(slot_index),
        function_name,
        resolved,
        context,
    )?);
    append_scope_drops_then_restore_aggregate_return(
        &mut instructions,
        slot_index,
        expected_layout,
        destination,
        function_return_type,
        context,
    )?;
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_value_return_with_scope_drops(
    success_type: &Type,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    mark_explicit_moves_in_expression(expression, context);
    if context.pending_aggregate_drops().is_empty() {
        return Ok(None);
    }

    let mut instructions = match success_type {
        Type::I32 => {
            let temporary = context.next_i32_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_i32_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::U8 => {
            let temporary = context.next_u8_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_u8_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Usize => {
            let temporary = context.next_usize_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_usize_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Bool => {
            let temporary = context.next_bool_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions = lower_bool_expression_to_location(
                expression,
                temporary,
                &expression_context,
                "E8007",
            )?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Str => {
            let temporary = context.next_str_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut instructions =
                lower_str_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(temporary),
                }],
                2,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Slice { .. } => {
            let temporary = context.next_slice_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut instructions =
                lower_slice_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(temporary),
                }],
                2,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Void
        | Type::Never
        | Type::Borrow { .. }
        | Type::Optional(_)
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. } => return Ok(None),
    };

    Ok(Some(std::mem::take(&mut instructions)))
}

pub(in crate::ir::lower) fn lower_aggregate_return_expression(
    expression: &Expr,
    return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (_, destination) = aggregate_return_layout_and_destination(return_type);
    let mut instructions = lower_aggregate_return_expression_to_location(
        expression,
        return_type,
        destination,
        function_name,
        resolved,
        context,
    )?;
    instructions.push(Instruction::Return);
    Ok(instructions)
}
