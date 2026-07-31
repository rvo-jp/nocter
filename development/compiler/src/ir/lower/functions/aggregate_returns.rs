use super::*;

pub(super) fn lower_terminal_aggregate_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_aggregate_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(super) fn lower_terminal_aggregate_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    };

    let then_instructions = lower_terminal_aggregate_return_block_with_prologue(
        &statement.then_block,
        context,
        &statement.condition,
        then_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_return_block_with_prologue(
        else_block,
        context,
        &statement.condition,
        else_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

pub(super) fn lower_terminal_aggregate_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_aggregate_switch_block(
            block,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_aggregate_switch_condition(
                condition,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )
        }
    }
}

pub(super) fn lower_terminal_aggregate_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(super) fn lower_terminal_aggregate_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_aggregate_switch_block(
        condition.then_branch,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_payloadless_switch_body(
        *condition.else_body,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

pub(super) fn lower_terminal_aggregate_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(super) fn lower_terminal_aggregate_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, "E8007", "functions", "aggregate")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        "E8007",
        "functions",
        "aggregate",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_terminal_aggregate_result_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &mut branch_context,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            if statement.expression.as_ref().is_some_and(|expression| {
                matches!(
                    unwrap_group(expression),
                    Expr::If(_) | Expr::IfIs(_) | Expr::Match(_)
                )
            }) {
                instructions.extend(lower_terminal_return_statement_with_scope_drops(
                    statement,
                    &mut branch_context,
                    "E8007",
                    "functions",
                    sources,
                )?);
                return Ok(instructions);
            }
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !branch_context.pending_aggregate_drops().is_empty()
            {
                instructions.extend(lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    &mut branch_context,
                )?);
                return Ok(instructions);
            }
            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &branch_context,
            )?;
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                &mut branch_context,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_aggregate_if_statement(
                statement,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is = tag_only_if_is_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = tag_only_switch_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_aggregate_if_diagnostic(function_name)),
    }
}

pub(super) fn lower_terminal_aggregate_result_expression(
    expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_aggregate_if_statement(
            statement,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007")?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007")?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        _ => {
            if let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(expression, context)?
            {
                mark_explicit_moves_in_expression(expression, context);
                return Ok(terminating_instructions);
            }

            mark_explicit_moves_in_expression(expression, context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !context.pending_aggregate_drops().is_empty()
            {
                return lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    context,
                );
            }

            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                context,
            )?;
            append_scope_end_drops_before_exit(return_instructions, context)
        }
    }
}

pub(super) fn lower_direct_aggregate_drop_instruction(
    name: &str,
    slot_index: usize,
    layout: ValueLayout,
    drop_glue: &crate::ir::lower::context::DropGlue,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1 || !drop_parameter_matches_local(&parameter_types[0], layout) {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(slot_index),
        })],
    })
}

pub(super) fn lower_payload_enum_drop_instructions(
    name: &str,
    slot_index: usize,
    drop_: &PayloadEnumDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let tag = temporaries.next_u8()?;
    let mut instructions = vec![Instruction::LoadAggregateU8 {
        destination: tag,
        source: AggregateLocation::Slot(slot_index),
        offset: 0,
    }];
    for variant in drop_.variants.iter().rev() {
        instructions.push(lower_payload_enum_drop_variant_if(
            name, slot_index, tag, variant, context,
        )?);
    }
    Ok(instructions)
}

pub(super) fn lower_payload_enum_drop_variant_if(
    name: &str,
    slot_index: usize,
    tag: U8Location,
    variant: &PayloadEnumDropVariant,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let mut then_instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        then_instructions.push(lower_payload_enum_drop_field(
            name, slot_index, field, context,
        )?);
    }

    Ok(Instruction::If {
        condition: BoolValue::I32Comparison {
            operator: I32ComparisonOperator::Equal,
            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(tag))),
            right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(variant.tag))),
        },
        then_instructions,
        else_instructions: Vec::new(),
    })
}

pub(super) fn lower_payload_enum_drop_field(
    name: &str,
    slot_index: usize,
    field: &PayloadEnumDropField,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&field.drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_local(&parameter_types[0], field.payload_layout)
    {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: field.drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index,
                offset: field.payload_offset,
            },
        })],
    })
}

pub(super) fn lower_aggregate_return_expression_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::StructLiteral(literal) => lower_aggregate_struct_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::ArrayLiteral(literal) => lower_aggregate_array_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Call(call) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                propagating_failure_mode(context)?,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                FallibleFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                lower_catch_failure_mode(catch, context, 0)?,
            )
        }
        Expr::Otherwise(otherwise) => lower_aggregate_otherwise_return_to_location(
            otherwise,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Identifier(identifier) => lower_aggregate_local_return_to_location(
            &identifier.name,
            AggregateValueUse::ImplicitCopy,
            return_type,
            destination,
            function_name,
            context,
        ),
        Expr::Member(_) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_member_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_local_return_to_location(
                &identifier.name,
                AggregateValueUse::ExplicitMove,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Group(group) => lower_aggregate_return_expression_to_location(
            &group.expression,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

pub(super) fn lower_aggregate_otherwise_return_to_location(
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

pub(super) fn lower_aggregate_otherwise_return_failure_mode(
    fallback: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        return_type,
        destination,
        function_name,
        resolved,
        &mut fallback_context,
    )?;
    if !exits {
        instructions.extend(append_scope_end_drops_before_exit(
            vec![Instruction::Return],
            &mut fallback_context,
        )?);
    }
    Ok(FallibleFailureMode::Handle { instructions })
}

pub(super) fn lower_aggregate_otherwise_fallback_to_location(
    block: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<(Vec<Instruction>, bool), Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions = lower_otherwise_return_leading_statements(block, context, "E8007")?;
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, context)?
        {
            instructions.extend(terminating_instructions);
            return Ok((instructions, true));
        }
        instructions.extend(lower_aggregate_return_expression_to_location(
            result,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        )?);
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
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok((instructions, true))
        }
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

pub(super) fn lower_aggregate_local_return_to_location(
    name: &str,
    value_use: AggregateValueUse,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(local) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if local.layout != expected_layout
        || (value_use == AggregateValueUse::ImplicitCopy && !local.is_copy)
    {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    Ok(vec![Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(local.slot_index),
        layout: local.layout,
    }])
}

pub(super) fn lower_aggregate_member_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let access = lower_aggregate_member_field_access(expression, context, &mut temporaries)?
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let Some(layout) = access.kind.copy_aggregate_layout() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if layout != expected_layout || !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_fallible_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if success_type.as_ref() != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(instructions)
}

pub(super) fn lower_aggregate_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
        if layout != expected_layout {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        }
        let mut temporaries = TemporaryAllocator::new(context)?;
        let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            destination,
            expected_layout,
            context,
            &mut temporaries,
        )?
        else {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        };
        return Ok(instructions);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(callee_return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if callee_return_type != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
    );
    Ok(instructions)
}

pub(super) fn lower_aggregate_struct_literal_return_to_location(
    literal: &StructLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_struct_literal_to_location(
        literal,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_struct_literal_return_through_slot(
                literal,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

pub(super) fn lower_aggregate_array_literal_return_to_location(
    literal: &ArrayLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = fixed_array_return_abi_value(resolved, context) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !matches!(&value.ty, AbiType::Array { .. }) || value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_array_literal_to_location(
        literal,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_array_literal_return_through_slot(
                literal,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

pub(super) fn fixed_array_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                return abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok();
            }
        }
    }
}

pub(super) fn payload_enum_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok()?;
                return matches!(value.ty, AbiType::Enum(_)).then_some(value);
            }
        }
    }
}

pub(super) fn lower_payload_enum_constructor_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = payload_enum_return_abi_value(resolved, context) else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    lower_payload_enum_constructor_value_to_location(
        expression,
        &value,
        expected_layout,
        destination,
        function_name,
        resolved,
        context,
    )
}

pub(super) fn lower_payload_enum_constructor_value_to_location(
    expression: &Expr,
    value: &AbiValue,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_payload_enum_constructor_to_location(
        expression,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    let instructions = match lowered_direct {
        Ok(Some(instructions)) => instructions,
        Ok(None) => return Ok(None),
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_payload_enum_constructor_return_through_slot(
                expression,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    };
    Ok(Some(instructions))
}

pub(super) fn lower_direct_payload_enum_constructor_return_through_slot(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    let Some(mut constructor_instructions) = lower_payload_enum_constructor_to_location(
        expression,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?
    else {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    };
    instructions.append(&mut constructor_instructions);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(super) fn lower_direct_aggregate_array_literal_return_through_slot(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_array_literal_to_location(
        literal,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(super) fn lower_direct_aggregate_struct_literal_return_through_slot(
    literal: &StructLiteralExpr,
    expected_layout: crate::abi::ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
        &mut temporaries,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

pub(super) fn aggregate_return_layout_and_destination(
    return_type: &Type,
) -> (crate::abi::ValueLayout, AggregateLocation) {
    match return_type {
        Type::Aggregate { layout } => (*layout, AggregateLocation::Return),
        Type::DirectAggregate { layout, .. } => (*layout, AggregateLocation::DirectReturn),
        _ => unreachable!("aggregate return lowering requires aggregate return type"),
    }
}

pub(super) fn validate_aggregate_call_success_return_passing(
    target: &CallTarget,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = context.call_success_return_passing(target) else {
        return Ok(());
    };
    let Some(expected) = return_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }

    Err(aggregate_call_return_abi_mismatch_diagnostic(
        function_name,
        expected,
        actual,
    ))
}

pub(super) fn aggregate_call_return_abi_mismatch_diagnostic(
    function_name: &str,
    expected: crate::abi::ReturnPassing,
    actual: crate::abi::ReturnPassing,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 aggregate return ABI mismatch in function `{function_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AggregateValueUse {
    ImplicitCopy,
    ExplicitMove,
}

pub(super) fn unsupported_aggregate_return_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower aggregate returns from function `{function_name}` from a supported struct literal, an aggregate call, or a supported aggregate local slot"
        ),
    )]
}

pub(super) fn macos_syscall_primitive_call(
    call: &crate::ast::CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}

pub(super) fn lower_aggregate_otherwise_return_failure_mode_with_scope_drops(
    fallback: &Block,
    success_type: &Type,
    function_return_type: &Type,
    slot_index: usize,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
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
    Ok(FallibleFailureMode::Handle { instructions })
}
