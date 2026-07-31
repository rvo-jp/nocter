use super::*;

pub(super) fn lower_callable_body(
    function_name: &str,
    body: &Block,
    return_type: &Type,
    _root_source: SourceId,
    resolved: &ResolveOutput,
    sources: &SourceMap,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let original_statements = body.statements.as_slice();

    if original_statements.iter().all(statement_is_import)
        && body.result.is_none()
        && *success_type == Type::Void
    {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let (statements, body_result) =
        reachable_body_prefix(original_statements, body.result.as_deref(), context);

    if let Some(result) = body_result {
        let mut instructions = lower_leading_bindings(statements, context, sources)?;
        instructions.extend(lower_callable_body_result(
            function_name,
            result,
            return_type,
            context,
            sources,
        )?);
        return Ok(instructions);
    }

    if success_type == &Type::Void
        && statements
            .iter()
            .rev()
            .find(|statement| !statement_is_import(statement))
            .is_some_and(statement_allows_implicit_void_return)
    {
        let mut instructions = lower_leading_bindings(statements, context, sources)?;
        instructions.extend(append_scope_end_drops_before_exit(
            vec![success_return_instruction(return_type)],
            context,
        )?);
        return Ok(instructions);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            body.span,
        ));
    };
    let mut instructions = lower_leading_bindings(leading, context, sources)?;

    match last {
        Stmt::Return(statement) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                context,
                "E8007",
                "functions",
                sources,
            )
            .map_err(|diagnostics| {
                let span = statement
                    .expression
                    .as_ref()
                    .map_or(statement.span, |expression| expression.span());
                attach_primary_span_if_absent(diagnostics, sources, span)
            })?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        Stmt::If(statement) => {
            let Some(branch_instructions) = lower_terminal_if_statement_for_success_type(
                statement,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                resolved,
                sources,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007").map_err(
                |diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                },
            )?;
            let Some(branch_instructions) =
                lower_terminal_if_statement_for_success_type_with_branch_prologues(
                    &if_is.statement,
                    context,
                    &if_is.then_prologue,
                    &BranchPrologue::empty(),
                    function_name,
                    return_type,
                    "E8007",
                    "functions",
                    resolved,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(if_is.leading_instructions);
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Switch(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007").map_err(
                |diagnostics| attach_primary_span_if_absent(diagnostics, sources, statement.span),
            )?;
            let Some(branch_instructions) = lower_terminal_payloadless_switch_for_success_type(
                switch,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                resolved,
                sources,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                context,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
            })?
            else {
                if success_type == &Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                {
                    instructions.extend(void_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, context);
                    instructions.extend(append_scope_end_drops_before_exit(
                        vec![success_return_instruction(return_type)],
                        context,
                    )?);
                    return Ok(instructions);
                }

                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        Stmt::Loop(statement) => {
            instructions.extend(
                lower_nonterminal_loop_statement(statement, context, "E8007", "functions", sources)
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
            );
            Ok(instructions)
        }
        _ => Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            last.span(),
        )),
    }
}

pub(super) fn lower_callable_body_result(
    function_name: &str,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_callable_control_body_result(
        function_name,
        expression,
        return_type,
        context,
        sources,
    )? {
        return Ok(instructions);
    }

    if return_type.success_type() == &Type::Void {
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(expression, context).map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            return Ok(terminating_instructions);
        }

        if let Some(mut void_instructions) = lower_void_expression_statement(expression, context)
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            mark_explicit_moves_in_expression(expression, context);
            void_instructions.extend(append_scope_end_drops_before_exit(
                vec![success_return_instruction(return_type)],
                context,
            )?);
            return Ok(void_instructions);
        }
    }

    let statement = ReturnStmt {
        span: expression.span(),
        expression: Some(expression.clone()),
    };
    lower_return_statement_with_scope_drops(&statement, context, "E8007")
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })
        .map_err(|diagnostics| {
            if diagnostics.is_empty() {
                attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    expression.span(),
                )
            } else {
                diagnostics
            }
        })
}

pub(super) fn lower_callable_control_body_result(
    function_name: &str,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => {
            lower_callable_if_body_result(statement, function_name, return_type, context, sources)
        }
        Expr::IfIs(statement) => {
            let mut control_context = context.clone();
            let if_is = tag_only_if_is_as_control_flow(statement, &mut control_context, "E8007")?;
            lower_callable_if_body_result_with_branch_prologues(
                &if_is.statement,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                function_name,
                return_type,
                &mut control_context,
                sources,
            )
            .map(|result| {
                result.map(|branch_instructions| {
                    let mut instructions = if_is.leading_instructions;
                    instructions.extend(branch_instructions);
                    instructions
                })
            })
        }
        Expr::Match(statement) => {
            let mut control_context = context.clone();
            let switch = tag_only_switch_as_control_flow(statement, &mut control_context, "E8007")?;
            lower_callable_payloadless_switch_body_result(
                switch,
                function_name,
                return_type,
                &mut control_context,
                sources,
            )
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_callable_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let resolved = context
        .resolved_calls()
        .map(|(_, resolved)| resolved)
        .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?;
    match lower_terminal_payloadless_switch_body_for_success_type(
        switch.body.clone(),
        context,
        function_name,
        return_type,
        "E8007",
        "functions",
        resolved,
        sources,
    ) {
        Ok(Some(mut branch_instructions)) => {
            let mut instructions = switch.leading_instructions;
            instructions.append(&mut branch_instructions);
            Ok(Some(mark_fallible_success_returns(
                return_type,
                instructions,
            )))
        }
        Ok(None) => Ok(None),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_callable_payloadless_switch_body_result(
                switch,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

pub(super) fn lower_void_nonterminal_callable_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = switch.leading_instructions;
    instructions.extend(lower_nonterminal_payloadless_switch_body(
        switch.body,
        context,
        None,
        &[],
        "E8007",
        "functions",
        sources,
    )?);
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

pub(super) fn lower_callable_if_body_result(
    statement: &IfStmt,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_callable_if_body_result_with_branch_prologues(
        statement,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        function_name,
        return_type,
        context,
        sources,
    )
}

pub(super) fn lower_callable_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let resolved = context
        .resolved_calls()
        .map(|(_, resolved)| resolved)
        .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?;
    match lower_terminal_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        function_name,
        return_type,
        "E8007",
        "functions",
        resolved,
        sources,
    ) {
        Ok(instructions) => Ok(instructions),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_callable_if_body_result_with_branch_prologues(
                statement,
                then_prologue,
                else_prologue,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

pub(super) fn lower_void_nonterminal_callable_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = lower_nonterminal_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        None,
        &[],
        "E8007",
        "functions",
        sources,
    )?;
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

pub(super) fn lower_terminal_if_statement_for_success_type(
    statement: &IfStmt,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_terminal_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )
}

pub(super) fn lower_terminal_if_statement_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) =
        lower_terminal_if_statement_body_for_success_type_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            function_name,
            return_type,
            diagnostic_code,
            subject,
            resolved,
            sources,
        )?
    else {
        return Ok(None);
    };

    Ok(Some(mark_fallible_success_returns(
        return_type,
        branch_instructions,
    )))
}

pub(super) fn lower_terminal_if_statement_body_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let branch_instructions = match success_type {
        Type::I32 => lower_terminal_i32_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Bool => lower_terminal_bool_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::U8 => lower_terminal_u8_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Usize => lower_terminal_usize_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Str => lower_terminal_str_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Slice { .. } => lower_terminal_slice_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Void => lower_terminal_void_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            lower_terminal_aggregate_if_statement_with_branch_prologues(
                statement,
                context,
                then_prologue,
                else_prologue,
                success_type,
                function_name,
                resolved,
                sources,
            )?
        }
        Type::Never | Type::Fallible(_) | Type::Borrow { .. } | Type::Error => return Ok(None),
    };

    Ok(Some(branch_instructions))
}

pub(super) fn lower_terminal_payloadless_switch_for_success_type(
    switch: LoweredPayloadlessSwitch,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) = lower_terminal_payloadless_switch_body_for_success_type(
        switch.body,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };

    let mut instructions = switch.leading_instructions;
    instructions.extend(branch_instructions);
    Ok(Some(mark_fallible_success_returns(
        return_type,
        instructions,
    )))
}

pub(super) fn lower_terminal_payloadless_switch_body_for_success_type(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => {
            lower_terminal_switch_block_for_success_type(
                block,
                context,
                function_name,
                return_type,
                diagnostic_code,
                subject,
                resolved,
                sources,
            )
        }
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_switch_condition_for_success_type(
                condition,
                context,
                function_name,
                return_type,
                diagnostic_code,
                subject,
                resolved,
                sources,
            )
        }
    }
}

pub(super) fn lower_terminal_switch_condition_for_success_type(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(then_instructions) = lower_terminal_switch_block_for_success_type(
        condition.then_branch,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };
    let Some(else_instructions) = lower_terminal_payloadless_switch_body_for_success_type(
        *condition.else_body,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )?))
}

pub(super) fn lower_terminal_switch_block_for_success_type(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let branch_instructions = match return_type.success_type() {
        Type::I32 => lower_terminal_i32_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Bool => lower_terminal_bool_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::U8 => lower_terminal_u8_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Usize => lower_terminal_usize_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Str => lower_terminal_str_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Slice { .. } => lower_terminal_slice_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Void => lower_terminal_void_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            lower_terminal_aggregate_switch_block(
                block,
                context,
                return_type.success_type(),
                function_name,
                resolved,
                sources,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(branch_instructions))
}
