use super::*;

pub(super) fn lower_catch_block(
    block: &Block,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((last, leading)) = block.statements.split_last() else {
        return Err(unsupported_catch_block_diagnostic());
    };

    let mut instructions = lower_catch_leading_statements(leading, context)?;
    let function_return_type = context.function_return_type().clone();
    let success_type = function_return_type.success_type().clone();

    match last {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_expression_with_scope_drops(expression, context)?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(
                    function_return_type,
                    Type::Fallible(_) | Type::ComposedOutcome { .. }
                )
                && let Some((root_source, resolved)) = context.resolved_calls()
                && let Some(payload) =
                    lower_error_payload(expression, resolved, root_source, Some(context))?
            {
                instructions.extend(append_scope_end_drops_before_exit(
                    lower_fallible_failure(payload),
                    context,
                )?);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && context.function_returns_optional()
                && expression_is_none_literal(expression)
            {
                instructions.extend(append_scope_end_drops_before_exit(
                    vec![Instruction::ReturnOptionalNone],
                    context,
                )?);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && let Some(return_instructions) = lower_value_return_with_scope_drops(
                    &success_type,
                    expression,
                    &function_return_type,
                    context,
                )?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(success_type, Type::DirectAggregate { .. })
                && !context.pending_aggregate_drops().is_empty()
            {
                let Some((_root_source, resolved)) = context.resolved_calls() else {
                    return Err(unsupported_catch_block_diagnostic());
                };
                let function_name = context.function_name().to_string();
                instructions.extend(lower_direct_aggregate_return_with_scope_drops(
                    expression,
                    &success_type,
                    &function_return_type,
                    &function_name,
                    resolved,
                    context,
                )?);
                return Ok(instructions);
            }

            let return_instructions = match (&success_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
                (Type::U8, Some(expression)) => lower_u8_return_expression(expression, context),
                (Type::Usize, Some(expression)) => {
                    lower_usize_return_expression(expression, context)
                }
                (Type::Bool, Some(expression)) => {
                    lower_bool_return_expression(expression, context, "E8007")
                }
                (Type::Str, Some(expression)) => lower_str_return_expression(expression, context),
                (Type::Slice { .. }, Some(expression)) => {
                    lower_slice_return_expression(expression, context)
                }
                (Type::Aggregate { .. } | Type::DirectAggregate { .. }, Some(expression)) => {
                    let Some((_root_source, resolved)) = context.resolved_calls() else {
                        return Err(unsupported_catch_block_diagnostic());
                    };
                    let function_name = context.function_name().to_string();
                    lower_aggregate_return_expression(
                        expression,
                        &success_type,
                        &function_name,
                        resolved,
                        context,
                    )
                }
                (Type::Void, None) => Ok(vec![Instruction::Return]),
                (Type::Void, Some(_)) => Err(unsupported_catch_block_diagnostic()),
                (Type::Never, Some(_)) => Err(unsupported_catch_block_diagnostic()),
                (Type::I32, None)
                | (Type::U8, None)
                | (Type::Usize, None)
                | (Type::Bool, None)
                | (Type::Str, None)
                | (Type::Slice { .. }, None)
                | (Type::Aggregate { .. }, None)
                | (Type::DirectAggregate { .. }, None)
                | (Type::Borrow { .. }, _)
                | (Type::Error, _)
                | (Type::Never, None) => Err(unsupported_catch_block_diagnostic()),
                (Type::Fallible(_) | Type::ComposedOutcome { .. }, _) => {
                    Err(unsupported_catch_block_diagnostic())
                }
            }?;
            let return_instructions =
                mark_fallible_success_returns(&function_return_type, return_instructions);
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                context,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                if success_type == Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context)?
                {
                    instructions.extend(void_instructions);
                    instructions.extend(append_scope_end_drops_before_exit(
                        vec![success_return_instruction(&function_return_type)],
                        context,
                    )?);
                    return Ok(instructions);
                }

                return Err(unsupported_catch_block_diagnostic());
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_catch_block_diagnostic()),
    }
}

pub(super) fn lower_catch_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_catch_block_diagnostic());
                };
                instructions.extend(void_instructions);
            }
            _ => return Err(unsupported_catch_block_diagnostic()),
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}

pub(super) fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    payload.into_return_instructions()
}

pub(super) fn expression_is_none_literal(expression: &Expr) -> bool {
    matches!(unwrap_group(expression), Expr::NoneLiteral(_))
}

pub(super) fn i32_destination_reserved_abi_words(destination: I32Location) -> usize {
    usize::from(matches!(destination, I32Location::Local(_)))
}

pub(super) fn u8_destination_reserved_abi_words(destination: U8Location) -> usize {
    usize::from(matches!(destination, U8Location::Local(_)))
}

pub(super) fn usize_destination_reserved_abi_words(destination: UsizeLocation) -> usize {
    usize::from(matches!(destination, UsizeLocation::Local(_)))
}

pub(super) fn bool_destination_reserved_abi_words(destination: BoolLocation) -> usize {
    usize::from(matches!(destination, BoolLocation::Local(_)))
}

pub(super) fn str_destination_reserved_abi_words(destination: StrLocation) -> usize {
    if matches!(destination, StrLocation::Local(_)) {
        2
    } else {
        0
    }
}

pub(super) fn slice_destination_reserved_abi_words(destination: SliceLocation) -> usize {
    if matches!(destination, SliceLocation::Local(_)) {
        2
    } else {
        0
    }
}

pub(super) fn lower_i32_fallible_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_i32_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_i32_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_i32_expression_diagnostic()),
    }
}

pub(super) fn lower_u8_fallible_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_u8_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_u8_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_u8_expression_diagnostic()),
    }
}

pub(super) fn lower_usize_fallible_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_usize_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_usize_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_usize_expression_diagnostic()),
    }
}

pub(super) fn lower_str_fallible_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_str_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_str_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_str_expression_diagnostic()),
    }
}

pub(super) fn lower_slice_fallible_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_slice_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_slice_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            failure_mode,
        ),
        _ => Err(unsupported_slice_expression_diagnostic()),
    }
}

pub(super) fn lower_bool_fallible_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_fallible_bool_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                failure_mode,
            )
        }
        Expr::Group(group) => lower_bool_fallible_expression_to_location(
            &group.expression,
            destination,
            context,
            diagnostic_code,
            failure_mode,
        ),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}
