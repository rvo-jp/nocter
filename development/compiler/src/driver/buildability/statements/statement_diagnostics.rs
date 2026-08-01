use super::*;

pub(in crate::driver::buildability) fn collect_statement_diagnostics(
    statement: &Stmt,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Region(statement) => diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            statement.keyword_span,
            "lexical `region` statements",
            "`region` is accepted by the v0.3 frontend, but runtime allocation-context lowering is not implemented yet",
        )),
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_terminal_return_expression_diagnostics(
                    expression,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Binding(statement) => {
            if let Some(diagnostic) = unsupported_local_binding_type_diagnostic(
                sources,
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            let binding_is_fixed_array_literal = fixed_array_literal_binding_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_is_scalar_or_view = binding_initializer_may_use_value_control_expression(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_type_expr = binding_type_expr_with_substitutions(
                statement,
                typecheck_facts,
                generic_substitutions,
            );
            let binding_fixed_array_type = binding_type_expr.as_ref().and_then(|ty| {
                fixed_array_type_abi_for_sources(ty, resolved, resolved_sources).map(|_| ty)
            });
            if let Expr::Otherwise(expression) = unwrap_group_expr(&statement.initializer) {
                collect_otherwise_binding_initializer_diagnostics(
                    expression,
                    binding_is_scalar_or_view,
                    binding_fixed_array_type,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if binding_is_fixed_array_literal
                || (binding_fixed_array_type.is_some()
                    && matches!(
                        unwrap_group_expr(&statement.initializer),
                        Expr::ArrayLiteral(_)
                    ))
            {
                collect_fixed_array_literal_binding_diagnostics(
                    statement,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if binding_is_scalar_or_view {
                collect_value_expression_diagnostics(
                    &statement.initializer,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &statement.initializer,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Assignment(statement) => {
            enqueue_member_replacement_drop_target(
                statement,
                typecheck_facts,
                generic_substitutions,
                root_source,
                queue,
            );
            if !assignment_operator_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.operator_span,
                    "compound assignment statements",
                    "use `i32`, `usize`, or `u8` whole-binding, aggregate-field, read-write slice element, or local/aggregate-field fixed-array element compound assignment, or use `target = target op value` until broader compound assignment lowering is promoted",
                ));
            }
            if let Some(diagnostic) = unsupported_index_assignment_target_diagnostic(
                sources,
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_fixed_array_assignment_diagnostic(
                sources,
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            let target_expression = match unwrap_group_expr(&statement.target) {
                Expr::Member(member)
                    if field_type_expr_for_member(member, resolved, typecheck_facts)
                        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
                        .is_some_and(|ty| {
                            type_expr_has_storage_only_scalar_abi_for_sources(
                                &ty,
                                resolved,
                                resolved_sources,
                            ) || fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
                                .is_some()
                        }) =>
                {
                    &member.object
                }
                _ => &statement.target,
            };
            collect_expression_diagnostics(
                target_expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            let assignment_is_fixed_array_literal = fixed_array_literal_assignment_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let assignment_fixed_array_type = fixed_array_assignment_target_type_expr(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            let assignment_targets_fixed_array = assignment_fixed_array_type.is_some();
            let assignment_aggregate_type = aggregate_assignment_target_type_expr(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .or(assignment_fixed_array_type);
            let assignment_is_scalar_or_view = assignment_value_may_use_value_control_expression(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
            if let Expr::Otherwise(otherwise) = unwrap_group_expr(&statement.value)
                && (assignment_is_scalar_or_view || assignment_aggregate_type.is_some())
            {
                collect_otherwise_assignment_value_diagnostics(
                    otherwise,
                    assignment_aggregate_type.as_ref(),
                    assignment_is_scalar_or_view,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if assignment_is_scalar_or_view {
                collect_value_expression_diagnostics(
                    &statement.value,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else if assignment_is_fixed_array_literal
                || (assignment_targets_fixed_array
                    && matches!(unwrap_group_expr(&statement.value), Expr::ArrayLiteral(_)))
            {
                collect_fixed_array_literal_elements_diagnostics(
                    unwrap_group_expr(&statement.value),
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &statement.value,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::If(statement) => {
            let exits_function = if_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            );
            if exits_function {
                collect_terminal_control_condition_move_diagnostics(
                    &statement.condition,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
            } else {
                collect_nonterminal_control_block_aggregate_diagnostics(
                    &statement.then_block,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        block,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
                collect_control_condition_move_diagnostics(
                    &statement.condition,
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
            }
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::IfIs(statement) => {
            if !if_is_statement_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                let diagnostic = unsupported_if_is_payload_binding_span(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                )
                .map(|span| unsupported_payload_binding_diagnostic(sources, span, "`if is`"))
                .unwrap_or_else(|| {
                    unsupported_v0_build_diagnostic(
                        sources,
                        statement.pattern_span,
                        "`if is` pattern branches",
                        "use payloadless enum patterns or supported payload bindings over existing values and owned call-expression, constructor, or move-local pattern targets",
                    )
                });
                diagnostics.push(diagnostic);
            }
            collect_payload_pattern_target_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if !if_is_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                collect_nonterminal_control_payload_block_aggregate_diagnostics(
                    &statement.then_block,
                    statement
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    sources,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    diagnostics,
                );
                if let Some(block) = &statement.else_block {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        block,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
            }
            collect_if_is_target_move_diagnostics(
                statement,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.then_block,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(block) = &statement.else_block {
                collect_block_diagnostics(
                    block,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::Switch(statement) => {
            if !switch_statement_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                let diagnostic = unsupported_switch_payload_binding_span(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                )
                .map(|span| unsupported_payload_binding_diagnostic(sources, span, "`match`"))
                .unwrap_or_else(|| {
                    unsupported_v0_build_diagnostic(
                        sources,
                        statement.span,
                        "`match` statements",
                        "use payloadless enum arms or supported payload bindings over existing values and owned call-expression, constructor, or move-local pattern targets",
                    )
                });
                diagnostics.push(diagnostic);
            }
            collect_payload_pattern_target_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if !switch_statement_exits_function_for_buildability(
                statement,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) {
                for arm in &statement.arms {
                    collect_nonterminal_control_payload_block_aggregate_diagnostics(
                        &arm.body,
                        arm.payload
                            .as_ref()
                            .and_then(|payload| payload.binding_name()),
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
                if let Some(arm) = &statement.wildcard_arm {
                    collect_nonterminal_control_block_aggregate_diagnostics(
                        &arm.body,
                        sources,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        diagnostics,
                    );
                }
            }
            collect_switch_target_move_diagnostics(
                statement,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            for arm in &statement.arms {
                collect_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
        }
        Stmt::ForRange(statement) => {
            if !range_for_binding_type_is_buildable(statement, typecheck_facts) {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.range_span,
                    "range `for` loops outside i32/usize bounds",
                    "use `i32` or `usize` bounds, or use `while` with explicit scalar state until broader range `for` lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &statement.start,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.end,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::While(statement) => {
            collect_control_condition_move_diagnostics(
                &statement.condition,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_expression_diagnostics(
                &statement.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Loop(statement) => {
            collect_nonterminal_control_block_aggregate_diagnostics(
                &statement.body,
                sources,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                diagnostics,
            );
            collect_block_diagnostics(
                &statement.body,
                return_type,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Expression(statement) => {
            if let Some(diagnostic) = unsupported_expression_statement_diagnostic(
                sources,
                &statement.expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &statement.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}
