use super::*;

pub(super) fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    payload.into_return_instructions()
}

pub(super) fn lower_otherwise_scalar_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    return_type: &Type,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if !otherwise_return_supports_success_type(success_type) {
        return Ok(None);
    }

    let Expr::Otherwise(otherwise) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Ok(None);
    }

    mark_explicit_moves_in_expression(&otherwise.value, context);
    let failure_mode =
        lower_otherwise_return_failure_mode(&otherwise.fallback, context, diagnostic_code)?;
    if !context.pending_aggregate_drops().is_empty() {
        let mut instructions = lower_otherwise_scalar_return_call_to_temporary(
            call,
            success_type,
            context,
            failure_mode,
        )?;
        append_scope_drops_then_restore_scalar_return(
            &mut instructions,
            success_type,
            return_type,
            context,
        )?;
        return Ok(Some(instructions));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut instructions = lower_otherwise_scalar_return_call_to_return(
        call,
        success_type,
        context,
        &mut temporaries,
        failure_mode,
    )?;
    instructions.push(success_return_instruction(return_type));
    append_scope_end_drops_before_exit(instructions, context).map(Some)
}

pub(super) fn otherwise_return_supports_success_type(success_type: &Type) -> bool {
    matches!(
        success_type,
        Type::I32 | Type::U8 | Type::Usize | Type::Bool | Type::Str | Type::Slice { .. }
    )
}

pub(super) fn lower_otherwise_scalar_return_call_to_return(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match success_type {
        Type::I32 => lower_fallible_i32_normal_call(
            call,
            I32Location::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::U8 => lower_fallible_u8_normal_call(
            call,
            U8Location::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Usize => lower_fallible_usize_normal_call(
            call,
            UsizeLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Bool => lower_fallible_bool_normal_call(
            call,
            BoolLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Str => lower_fallible_str_normal_call(
            call,
            StrLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Slice { .. } => lower_fallible_slice_normal_call(
            call,
            SliceLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Optional(_)
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. } => Err(vec![Diagnostic::error(
            "E8007",
            "native lowering can only lower `otherwise` returns for scalar success types",
        )]),
    }
}

pub(super) fn lower_otherwise_scalar_return_call_to_temporary(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match success_type {
        Type::I32 => {
            let destination = context.next_i32_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_i32_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::U8 => {
            let destination = context.next_u8_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_u8_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Usize => {
            let destination = context.next_usize_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_usize_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Bool => {
            let destination = context.next_bool_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_bool_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Str => {
            let destination = context.next_str_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_str_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Slice { .. } => {
            let destination = context.next_slice_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_slice_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Optional(_)
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. } => Err(vec![Diagnostic::error(
            "E8007",
            "native lowering can only lower `otherwise` returns for scalar success types",
        )]),
    }
}

pub(super) fn append_scope_drops_then_restore_scalar_return(
    instructions: &mut Vec<Instruction>,
    success_type: &Type,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let restore_return = match success_type {
        Type::I32 => vec![Instruction::SetI32 {
            destination: I32Location::Return,
            value: I32Value::Location(context.next_i32_local_location()?),
        }],
        Type::U8 => vec![Instruction::SetU8 {
            destination: U8Location::Return,
            value: U8Value::Location(context.next_u8_local_location()?),
        }],
        Type::Usize => vec![Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Location(context.next_usize_local_location()?),
        }],
        Type::Bool => vec![Instruction::SetBool {
            destination: BoolLocation::Return,
            value: BoolValue::Location(context.next_bool_local_location()?),
        }],
        Type::Str => vec![Instruction::SetStr {
            destination: StrLocation::Return,
            value: StrValue::Location(context.next_str_local_location()?),
        }],
        Type::Slice { .. } => vec![Instruction::SetSlice {
            destination: SliceLocation::Return,
            value: SliceValue::Location(context.next_slice_local_location()?),
        }],
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Optional(_)
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. } => {
            return Err(vec![Diagnostic::error(
                "E8007",
                "native lowering can only restore `otherwise` returns for scalar success types",
            )]);
        }
    };
    append_scope_drops_then_restore_return(
        instructions,
        restore_return,
        scalar_return_temporary_abi_words(success_type)?,
        return_type,
        context,
    )
}

pub(super) fn scalar_return_temporary_abi_words(
    success_type: &Type,
) -> Result<usize, Vec<Diagnostic>> {
    match success_type {
        Type::I32 | Type::U8 | Type::Usize | Type::Bool => Ok(1),
        Type::Str | Type::Slice { .. } => Ok(2),
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Optional(_)
        | Type::Fallible(_)
        | Type::ComposedOutcome { .. } => Err(vec![Diagnostic::error(
            "E8007",
            "native lowering can only restore `otherwise` returns for scalar success types",
        )]),
    }
}

pub(super) fn lower_otherwise_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_return_type: &Type,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if context.pending_aggregate_drops().is_empty() {
        return Ok(None);
    }
    let Some(layout) = aggregate_type_layout(success_type) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }

    let Expr::Otherwise(otherwise) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Ok(None);
    }

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    };
    let function_name = context.function_name().to_string();
    let (_, destination) = aggregate_return_layout_and_destination(success_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let staged_destination = AggregateLocation::Slot(slot_index);
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];

    mark_explicit_moves_in_expression(&otherwise.value, context);
    let failure_mode = lower_aggregate_otherwise_return_failure_mode_with_scope_drops(
        &otherwise.fallback,
        success_type,
        function_return_type,
        slot_index,
        destination,
        &function_name,
        resolved,
        context,
    )?;
    instructions.extend(lower_aggregate_fallible_call_return_to_location(
        call,
        success_type,
        staged_destination,
        &function_name,
        context,
        failure_mode,
    )?);
    append_scope_drops_then_restore_aggregate_return(
        &mut instructions,
        slot_index,
        layout,
        destination,
        function_return_type,
        context,
    )?;
    Ok(Some(instructions))
}

pub(super) fn append_scope_drops_then_restore_aggregate_return(
    instructions: &mut Vec<Instruction>,
    slot_index: usize,
    layout: ValueLayout,
    destination: AggregateLocation,
    function_return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let mut tail = append_scope_end_drops_before_exit(
        vec![success_return_instruction(function_return_type)],
        context,
    )?;
    let Some(return_index) = tail.iter().rposition(is_scope_exit_instruction) else {
        return Ok(());
    };
    tail.insert(
        return_index,
        Instruction::CopyAggregate {
            destination,
            source: AggregateLocation::Slot(slot_index),
            layout,
        },
    );
    instructions.extend(tail);
    Ok(())
}

pub(super) fn call_return_type_expr_is_top_level_optional(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return false;
    };
    return_type_expr_is_top_level_optional_with_resolver(&return_type, resolved, |source| {
        context.resolved_source(source)
    })
}

pub(super) fn lower_otherwise_return_failure_mode(
    fallback: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let instructions =
        lower_otherwise_return_block(fallback, &mut fallback_context, diagnostic_code)?;
    Ok(OutcomeFailureMode::Handle { instructions })
}

pub(super) fn lower_otherwise_return_block(
    block: &Block,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions =
            lower_otherwise_return_leading_statements(block, context, diagnostic_code)?;
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, context)?
        {
            instructions.extend(terminating_instructions);
            return Ok(instructions);
        }
        let fallback_return = ReturnStmt {
            span: result.span(),
            expression: Some((**result).clone()),
        };
        instructions.extend(lower_return_statement_with_scope_drops(
            &fallback_return,
            context,
            diagnostic_code,
        )?);
        return Ok(instructions);
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
    };
    let mut instructions =
        lower_otherwise_return_statement_prefix(leading, context, diagnostic_code)?;
    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement,
                context,
                diagnostic_code,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code)),
    }
}

pub(super) fn lower_otherwise_return_leading_statements(
    block: &Block,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_otherwise_return_statement_prefix(&block.statements, context, diagnostic_code)
}

pub(super) fn lower_otherwise_return_statement_prefix(
    statements: &[Stmt],
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?)
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?)
            }
            Stmt::Drop(statement) => instructions.extend(lower_drop_statement(statement, context)?),
            Stmt::Expression(statement) => {
                let Some(effect) = lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
                };
                instructions.extend(effect);
            }
            _ => return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code)),
        }
    }
    Ok(instructions)
}

pub(super) fn unsupported_otherwise_fallback_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "native lowering can only lower `otherwise` fallback blocks with local bindings, assignments, drops, effect-only calls, and a value, `return`, or `never` tail",
    )]
}
