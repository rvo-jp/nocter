use super::*;

pub(in crate::ir::lower) fn lower_nonterminal_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_nonterminal_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_nonterminal_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            statement.condition.span(),
        ));
    }

    let then_instructions = lower_nonterminal_if_block_with_prologue(
        &statement.then_block,
        context,
        then_prologue,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = if let Some(else_block) = &statement.else_block {
        lower_nonterminal_if_block_with_prologue(
            else_block,
            context,
            else_prologue,
            loop_scope_mark,
            continue_instructions,
            diagnostic_code,
            subject,
            sources,
        )?
    } else {
        Vec::new()
    };

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_nonterminal_payloadless_switch_statement(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let switch = tag_only_switch_as_control_flow(statement, context, diagnostic_code)?;
    let target_cleanup = switch.target_cleanup;
    let mut instructions = switch.leading_instructions;
    instructions.extend(lower_nonterminal_payloadless_switch_body(
        switch.body,
        context,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?);
    if !instruction_list_ends_execution(&instructions)
        && let Some(cleanup) = target_cleanup
    {
        cleanup.append_to(&mut instructions, context)?;
    }
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_nonterminal_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_nonterminal_if_block_with_prologue(
            &block.block,
            context,
            &block.prologue,
            loop_scope_mark,
            continue_instructions,
            diagnostic_code,
            subject,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => lower_nonterminal_switch_condition(
            condition,
            context,
            loop_scope_mark,
            continue_instructions,
            diagnostic_code,
            subject,
            sources,
        ),
    }
}

fn lower_nonterminal_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&condition.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            condition.condition.span(),
        ));
    }

    let then_instructions = lower_nonterminal_if_block_with_prologue(
        &condition.then_branch.block,
        context,
        &condition.then_branch.prologue,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_nonterminal_payloadless_switch_body(
        *condition.else_body,
        context,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;

    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_nonterminal_while_statement(
    statement: &WhileStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            statement.condition.span(),
        ));
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, diagnostic_code)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.condition.span())
        })?;
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject, sources)?;

    Ok(vec![Instruction::While {
        condition_instructions: condition.instructions,
        condition: condition.value,
        body_instructions,
    }])
}

pub(in crate::ir::lower) fn lower_nonterminal_loop_statement(
    statement: &LoopStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject, sources)?;

    Ok(vec![Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::Const(true),
        body_instructions,
    }])
}

pub(in crate::ir::lower) fn lower_nonterminal_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match context.binding_scalar_view_kind(statement.name_span) {
        Some(TypecheckScalarViewKind::I32) => lower_nonterminal_i32_for_range_statement(
            statement,
            context,
            diagnostic_code,
            subject,
            sources,
        ),
        Some(TypecheckScalarViewKind::Usize) => lower_nonterminal_usize_for_range_statement(
            statement,
            context,
            diagnostic_code,
            subject,
            sources,
        ),
        _ => Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}

fn lower_nonterminal_i32_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value_hidden = hidden_for_range_local_name(statement, "value");
    let end_hidden = hidden_for_range_local_name(statement, "end");
    let value = context.next_i32_local_location()?;
    context.define_i32_local(value_hidden.clone());
    let end = context.next_i32_local_location()?;
    context.define_i32_local(end_hidden);

    let mut instructions = lower_i32_expression_to_location(&statement.start, value, context)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.start.span())
        })?;
    instructions.extend(
        lower_i32_expression_to_location(&statement.end, end, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.end.span())
        })?,
    );
    if !context.rename_local(&value_hidden, statement.name.clone()) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let increment = vec![Instruction::AddI32 {
        destination: value,
        left: I32Value::Location(value),
        right: I32Value::Const(1),
    }];
    let body_instructions = lower_nonterminal_for_range_block(
        &statement.body,
        context,
        &increment,
        diagnostic_code,
        subject,
        sources,
    )?;
    if !context.rename_local(&statement.name, value_hidden) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    instructions.push(Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::I32Comparison {
            operator: I32ComparisonOperator::Less,
            left: I32Value::Location(value),
            right: I32Value::Location(end),
        },
        body_instructions,
    });
    Ok(instructions)
}

fn lower_nonterminal_usize_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value_hidden = hidden_for_range_local_name(statement, "value");
    let end_hidden = hidden_for_range_local_name(statement, "end");
    let value = context.next_usize_local_location()?;
    context.define_usize_local(value_hidden.clone());
    let end = context.next_usize_local_location()?;
    context.define_usize_local(end_hidden);

    let mut instructions = lower_usize_expression_to_location(&statement.start, value, context)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.start.span())
        })?;
    instructions.extend(
        lower_usize_expression_to_location(&statement.end, end, context).map_err(
            |diagnostics| attach_primary_span_if_absent(diagnostics, sources, statement.end.span()),
        )?,
    );
    if !context.rename_local(&value_hidden, statement.name.clone()) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let increment = vec![Instruction::AddUsize {
        destination: value,
        left: UsizeValue::Location(value),
        right: UsizeValue::Const(1),
    }];
    let body_instructions = lower_nonterminal_for_range_block(
        &statement.body,
        context,
        &increment,
        diagnostic_code,
        subject,
        sources,
    )?;
    if !context.rename_local(&statement.name, value_hidden) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    instructions.push(Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::UsizeComparison {
            operator: I32ComparisonOperator::Less,
            left: UsizeValue::Location(value),
            right: UsizeValue::Location(end),
        },
        body_instructions,
    });
    Ok(instructions)
}

fn lower_nonterminal_for_range_block(
    block: &Block,
    context: &LoweringContext,
    increment_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let region_mark = body_context.region_cleanup_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut body_context,
        local_mark,
        Some(CleanupScopeMark {
            locals: local_mark,
            regions: region_mark,
        }),
        increment_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
        instructions.extend(increment_instructions.iter().cloned());
    }
    Ok(instructions)
}

fn hidden_for_range_local_name(statement: &ForRangeStmt, role: &str) -> String {
    format!(
        "<for-range:{}:{}:{role}>",
        statement.name_span.start, statement.name_span.end
    )
}

fn lower_nonterminal_while_block(
    block: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let region_mark = body_context.region_cleanup_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut body_context,
        local_mark,
        Some(CleanupScopeMark {
            locals: local_mark,
            regions: region_mark,
        }),
        &[],
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

fn lower_nonterminal_if_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    prologue: &BranchPrologue,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let mut instructions = prologue.apply(&mut branch_context)?;
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut branch_context,
        local_mark,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    instructions.extend(lowered.instructions);
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut branch_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_nonterminal_loop_block_statements(
    statements: &[Stmt],
    result: Option<&Expr>,
    context: &mut LoweringContext,
    local_mark: usize,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<LoweredNonterminalBlock, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut ends_execution = false;
    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) && !outer_aggregate_move_binding_before_function_exit_allowed(
                    statement, context, local_mark, statements, index, result,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.initializer.span(),
                    ));
                }
                let loop_control = loop_scope_mark.map(|scope_mark| LoopControlContext {
                    scope_mark,
                    continue_instructions,
                });
                instructions.extend(
                    lower_local_binding_with_loop_control(statement, context, loop_control)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(diagnostics, sources, statement.span)
                        })?,
                )
            }
            Stmt::Assignment(statement) => {
                let target_allowed =
                    nonterminal_assignment_target_allowed(statement, context, local_mark)
                        || outer_aggregate_assignment_before_function_exit_allowed(
                            statement, context, local_mark, statements, index, result,
                        );
                let explicit_outer_aggregate_move_allowed =
                    aggregate_move_assignment_before_function_exit_allowed(
                        statement, context, local_mark, statements, index, result,
                    );
                if !target_allowed
                    || (expression_contains_explicit_aggregate_move_outside(
                        &statement.value,
                        context,
                        local_mark,
                    ) && !explicit_outer_aggregate_move_allowed)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.expression.span(),
                    ));
                }
                if let Some(terminating_instructions) =
                    lower_never_expression_with_scope_drops(&statement.expression, context)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(
                                diagnostics,
                                sources,
                                statement.expression.span(),
                            )
                        })?
                {
                    instructions.extend(terminating_instructions);
                    ends_execution = true;
                } else {
                    let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                    else {
                        return Err(attach_primary_span_if_absent(
                            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                            sources,
                            statement.span,
                        ));
                    };
                    instructions.extend(void_instructions);
                }
            }
            Stmt::Drop(statement) => {
                if !context.aggregate_local_defined_since(&statement.name, local_mark)
                    && !statement_suffix_exits_function(statements, index, result, context)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Return(statement) => {
                instructions.extend(
                    lower_terminal_return_statement_with_scope_drops(
                        statement,
                        context,
                        diagnostic_code,
                        subject,
                        sources,
                    )
                    .map_err(|diagnostics| {
                        let span = statement
                            .expression
                            .as_ref()
                            .map_or(statement.span, |expression| expression.span());
                        attach_primary_span_if_absent(diagnostics, sources, span)
                    })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::If(statement) => {
                let lowered = lower_nonterminal_if_statement(
                    statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::IfIs(statement) => {
                let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                    })?;
                let target_cleanup = if_is.target_cleanup;
                let lowered = lower_nonterminal_if_statement_with_branch_prologues(
                    &if_is.statement,
                    context,
                    &if_is.then_prologue,
                    &BranchPrologue::empty(),
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                let mut lowered_with_condition = if_is.leading_instructions;
                lowered_with_condition.extend(lowered);
                ends_execution = instruction_list_ends_execution(&lowered_with_condition);
                if !ends_execution && let Some(cleanup) = target_cleanup {
                    cleanup.append_to(&mut lowered_with_condition, context)?;
                }
                instructions.extend(lowered_with_condition);
            }
            Stmt::Switch(statement) => {
                let lowered = lower_nonterminal_payloadless_switch_statement(
                    statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::While(statement) => instructions.extend(
                lower_nonterminal_while_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::ForRange(statement) => instructions.extend(
                lower_nonterminal_for_range_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::CollectionFor(statement) => instructions.extend(
                crate::ir::lower::collection_for::lower_collection_for_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::LiteralPackFor(statement) => {
                instructions.extend(
                    crate::ir::lower::literal_packs::lower_literal_pack_for_statement(
                        statement,
                        context,
                        diagnostic_code,
                        subject,
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Loop(statement) => instructions.extend(
                lower_nonterminal_loop_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Region(statement) => {
                let lowered = lower_nonterminal_region_statement(
                    statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::Break(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Break,
                        context,
                        loop_scope_mark,
                        &[],
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::Continue(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Continue,
                        context,
                        loop_scope_mark,
                        continue_instructions,
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
        if ends_execution {
            break;
        }
    }
    if !ends_execution && let Some(result) = result {
        instructions.extend(lower_nonterminal_block_result(
            result,
            context,
            local_mark,
            diagnostic_code,
            subject,
            sources,
        )?);
        ends_execution = expression_exits_function(result, context);
    }
    Ok(LoweredNonterminalBlock {
        instructions,
        ends_execution,
    })
}

pub(in crate::ir::lower) fn lower_nonterminal_region_body(
    block: &Block,
    context: &mut LoweringContext,
    local_mark: usize,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<LoweredNonterminalBlock, Vec<Diagnostic>> {
    lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        context,
        local_mark,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )
}

fn lower_nonterminal_block_result(
    expression: &Expr,
    context: &mut LoweringContext,
    local_mark: usize,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move_outside(expression, context, local_mark) {
        return Err(attach_primary_span_if_absent(
            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
            sources,
            expression.span(),
        ));
    }

    if let Some(terminating_instructions) =
        lower_never_expression_with_scope_drops(expression, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })?
    {
        mark_explicit_moves_in_expression(expression, context);
        return Ok(terminating_instructions);
    }

    let Some(void_instructions) =
        lower_void_expression_statement(expression, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })?
    else {
        return Err(attach_primary_span_if_absent(
            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
            sources,
            expression.span(),
        ));
    };
    mark_explicit_moves_in_expression(expression, context);
    Ok(void_instructions)
}
