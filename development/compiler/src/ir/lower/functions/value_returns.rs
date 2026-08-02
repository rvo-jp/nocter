use super::*;

pub(super) fn lower_terminal_control_return_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let return_type = context.function_return_type().clone();
    let function_name = context.function_name().to_string();
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_if_statement_for_success_type(
            statement,
            context,
            &function_name,
            &return_type,
            diagnostic_code,
            subject,
            context
                .resolved_calls()
                .map(|(_, resolved)| resolved)
                .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
            sources,
        ),
        Expr::IfIs(statement) => {
            let mut control_context = context.clone();
            let if_is =
                tag_only_if_is_as_control_flow(statement, &mut control_context, diagnostic_code)?;
            lower_terminal_if_statement_for_success_type_with_branch_prologues(
                &if_is.statement,
                &control_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                &function_name,
                &return_type,
                diagnostic_code,
                subject,
                control_context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
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
            let switch =
                tag_only_switch_as_control_flow(statement, &mut control_context, diagnostic_code)?;
            lower_terminal_payloadless_switch_for_success_type(
                switch,
                &control_context,
                &function_name,
                &return_type,
                diagnostic_code,
                subject,
                control_context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
                sources,
            )
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_terminal_direct_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(success_type);
    if !matches!(destination, AggregateLocation::DirectReturn)
        || !supported_aggregate_copy_layout(expected_layout)
    {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
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

    let mut tail = append_scope_end_drops_before_exit(vec![Instruction::Return], context)?;
    let Some(return_index) = tail
        .iter()
        .rposition(|instruction| matches!(instruction, Instruction::Return))
    else {
        return Ok(instructions);
    };
    tail.insert(
        return_index,
        Instruction::CopyAggregate {
            destination,
            source: AggregateLocation::Slot(slot_index),
            layout: expected_layout,
        },
    );
    instructions.extend(tail);
    Ok(instructions)
}

pub(super) fn append_scope_drops_then_restore_return(
    instructions: &mut Vec<Instruction>,
    restore_return: Vec<Instruction>,
    reserved_local_abi_words: usize,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let cleanup_context = context.with_reserved_local_abi_words(reserved_local_abi_words);
    let mut tail = vec![success_return_instruction(return_type)];
    let Some(return_index) = tail.iter().rposition(is_scope_exit_instruction) else {
        return Ok(());
    };
    let drops = lower_scope_end_drop_instructions(&cleanup_context)?;
    let restore_index = return_index + drops.len();
    tail.splice(return_index..return_index, drops);
    mark_pending_aggregate_drops(context);
    tail.splice(restore_index..restore_index, restore_return);
    instructions.extend(tail);
    Ok(())
}

pub(super) fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) = lower_void_expression_statement(
                    &statement.expression,
                    context,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
                })?
                else {
                    return Err(attach_primary_span_if_absent(
                        vec![Diagnostic::error(
                            "E8007",
                            "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, or effect-only call statements before `return`",
                        )],
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::If(statement) => {
                instructions.extend(
                    lower_nonterminal_if_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::IfIs(statement) => {
                let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007").map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                    },
                )?;
                let target_cleanup = if_is.target_cleanup;
                instructions.extend(if_is.leading_instructions);
                instructions.extend(
                    lower_nonterminal_if_statement_with_branch_prologues(
                        &if_is.statement,
                        context,
                        &if_is.then_prologue,
                        &BranchPrologue::empty(),
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
                if let Some(cleanup) = target_cleanup {
                    cleanup.append_to(&mut instructions, context)?;
                }
            }
            Stmt::Switch(statement) => {
                instructions.extend(
                    lower_nonterminal_payloadless_switch_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::ForRange(statement) => {
                instructions.extend(
                    lower_nonterminal_for_range_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::LiteralPackFor(statement) => {
                instructions.extend(
                    crate::ir::lower::literal_packs::lower_literal_pack_for_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::While(statement) => {
                instructions.extend(
                    lower_nonterminal_while_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Loop(statement) => {
                instructions.extend(
                    lower_nonterminal_loop_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Region(statement) => {
                instructions.extend(
                    lower_nonterminal_region_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    vec![Diagnostic::error(
                        "E8007",
                        "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, effect-only call statements, or supported non-terminal `if`/`for`/`while`/`loop` statements before `return`",
                    )],
                    sources,
                    statement.span(),
                ));
            }
        };
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}
