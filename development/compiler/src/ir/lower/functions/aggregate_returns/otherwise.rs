use super::*;

pub(in crate::ir::lower::functions) fn lower_aggregate_otherwise_return_to_location(
    otherwise: &crate::ast::OtherwiseExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !context.pending_aggregate_drops().is_empty() {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let failure_mode = lower_aggregate_otherwise_return_failure_mode(
        &otherwise.fallback,
        return_type,
        destination,
        function_name,
        resolved,
        context,
    )?;
    lower_aggregate_fallible_call_return_to_location(
        call,
        return_type,
        destination,
        function_name,
        context,
        failure_mode,
    )
}

pub(in crate::ir::lower::functions) fn lower_aggregate_otherwise_value_to_location(
    otherwise: &crate::ast::OtherwiseExpr,
    value: &AbiValue,
    value_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let failure_mode = lower_aggregate_otherwise_value_failure_mode(
        &otherwise.fallback,
        value_type,
        destination,
        function_name,
        resolved,
        context,
        value,
    )?;
    lower_aggregate_fallible_call_return_to_location(
        call,
        value_type,
        destination,
        function_name,
        context,
        failure_mode,
    )
}

fn lower_aggregate_otherwise_value_failure_mode(
    fallback: &Block,
    value_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    value: &AbiValue,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let local_mark = fallback_context.local_mark();
    mark_explicit_moves_in_block(fallback, &mut fallback_context);
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        value_type,
        destination,
        function_name,
        resolved,
        &mut fallback_context,
        Some(value),
    )?;
    if exits {
        return Ok(OutcomeFailureMode::Handle { instructions });
    }
    instructions.extend(lower_scope_end_drops_for_locals_since(
        &mut fallback_context,
        local_mark,
    )?);
    Ok(OutcomeFailureMode::Recover { instructions })
}

pub(in crate::ir::lower::functions) fn lower_aggregate_otherwise_return_failure_mode(
    fallback: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        return_type,
        destination,
        function_name,
        resolved,
        &mut fallback_context,
        None,
    )?;
    if !exits {
        instructions.extend(append_scope_end_drops_before_exit(
            vec![Instruction::Return],
            &mut fallback_context,
        )?);
    }
    Ok(OutcomeFailureMode::Handle { instructions })
}

pub(in crate::ir::lower::functions) fn lower_aggregate_otherwise_fallback_to_location(
    block: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
    payload_enum_value: Option<&AbiValue>,
) -> Result<(Vec<Instruction>, bool), Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions = lower_otherwise_return_leading_statements(block, context, "E8007")?;
        if let Some(terminating_instructions) = lower_never_expression(result, context)? {
            instructions.extend(terminating_instructions);
            return Ok((instructions, true));
        }
        let lowered_constructor = payload_enum_value
            .map(|value| {
                lower_payload_enum_constructor_value_to_location(
                    result,
                    value,
                    value.layout,
                    destination,
                    function_name,
                    resolved,
                    context,
                )
            })
            .transpose()?
            .flatten();
        if let Some(constructor_instructions) = lowered_constructor {
            instructions.extend(constructor_instructions);
        } else {
            instructions.extend(lower_aggregate_return_expression_to_location(
                result,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )?);
        }
        mark_explicit_moves_in_expression(result, context);
        return Ok((instructions, false));
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let mut instructions = lower_otherwise_return_statement_prefix(leading, context, "E8007")?;
    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement, context, "E8007",
            )?);
            Ok((instructions, true))
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression(&statement.expression, context)?
            else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok((instructions, true))
        }
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

pub(in crate::ir::lower::functions) fn lower_aggregate_otherwise_return_failure_mode_with_scope_drops(
    fallback: &Block,
    success_type: &Type,
    function_return_type: &Type,
    slot_index: usize,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    mark_explicit_moves_in_block(fallback, &mut fallback_context);
    let layout = aggregate_type_layout(success_type)
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        success_type,
        AggregateLocation::Slot(slot_index),
        function_name,
        resolved,
        &mut fallback_context,
        None,
    )?;
    if !exits {
        append_scope_drops_then_restore_aggregate_return(
            &mut instructions,
            slot_index,
            layout,
            destination,
            function_return_type,
            &mut fallback_context,
        )?;
    }
    Ok(OutcomeFailureMode::Handle { instructions })
}
