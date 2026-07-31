use super::*;

pub(super) fn assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(target) {
        Expr::Identifier(identifier) => Some(local_identifier_type_expr_with_substitutions(
            identifier,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )?),
        Expr::Member(member) => {
            let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
            Some(substitute_type_expr_parameters(&ty, generic_substitutions))
        }
        _ => None,
    }
}

pub(super) fn collect_statement_diagnostics(
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
            collect_expression_diagnostics(
                &statement.target,
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
            let assignment_targets_fixed_array = fixed_array_assignment_target_abi(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .is_some();
            let assignment_aggregate_type = aggregate_assignment_target_type_expr(
                &statement.target,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            );
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
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.pattern_span,
                    "`if is` pattern branches",
                    "use payloadless enum patterns or tag-only payload enum patterns over existing values and supported call/constructor/move-local pattern targets, or keep unsupported payload binding code on the `check` path",
                ));
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
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "`match` statements",
                    "use payloadless enum `match` arms or tag-only payload enum discard arms over existing values, or keep payload binding code on the `check` path",
                ));
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

pub(super) fn collect_control_condition_move_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(span) = expression_explicit_aggregate_move_span(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return;
    };

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "select the branch before moving aggregate values until control-flow condition move lowering is promoted",
    ));
}

pub(super) fn collect_nonterminal_control_block_aggregate_diagnostics(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
        block,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        HashSet::new(),
        diagnostics,
    );
}

pub(super) fn collect_nonterminal_control_payload_block_aggregate_diagnostics(
    block: &Block,
    payload_name: Option<&str>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_bindings = HashSet::new();
    if let Some(payload_name) = payload_name {
        local_bindings.insert(payload_name.to_owned());
    }
    collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
        block,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
        diagnostics,
    );
}

pub(super) fn collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
    block: &Block,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    mut local_bindings: HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Stmt::Binding(statement) => {
                if let Some(span) = unsupported_outer_aggregate_move_binding_span(
                    statement,
                    statements,
                    index,
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or move outer values only into bindings/assignments on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
                local_bindings.insert(statement.name.clone());
            }
            Stmt::Assignment(statement) => {
                if let Some(span) = unsupported_outer_aggregate_move_assignment_span(
                    statement,
                    statements,
                    index,
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or move outer values only into bindings/assignments on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
            }
            Stmt::Expression(statement) => {
                if let Some(span) = expression_explicit_outer_aggregate_move_span(
                    &statement.expression,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    &local_bindings,
                ) {
                    diagnostics.push(unsupported_v0_build_diagnostic(
                        sources,
                        span,
                        "explicit outer aggregate moves inside non-terminal control flow",
                        "move values created inside the branch/body, or bind or assign outer moves only on paths that immediately exit the function until broader control-flow move lowering is promoted",
                    ));
                }
            }
            Stmt::Drop(statement)
                if !local_bindings.contains(&statement.name)
                    && !statement_suffix_exits_function_for_buildability(
                        statements,
                        index,
                        result,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                    ) =>
            {
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    statement.span,
                    "explicit outer aggregate drops inside non-terminal control flow",
                    "drop values created inside the branch/body, or drop outer values only on paths that immediately exit the function until broader control-flow drop lowering is promoted",
                ));
            }
            _ => {}
        }
    }
    if let Some(result) = result
        && let Some(span) = expression_explicit_outer_aggregate_move_span(
            result,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            &local_bindings,
        )
    {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            span,
            "explicit outer aggregate moves inside non-terminal control-flow results",
            "move values created inside the branch/body, or move outer values only before a statement that immediately exits the function until broader control-flow move lowering is promoted",
        ));
    }
}

pub(super) fn unsupported_outer_aggregate_move_binding_span(
    statement: &BindingStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if direct_outer_aggregate_move_for_buildability(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }
    Some(span)
}

pub(super) fn unsupported_outer_aggregate_move_assignment_span(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
        statement,
        statements,
        index,
        result,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) {
        return None;
    }
    Some(span)
}

pub(super) fn assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    direct_outer_aggregate_move_for_buildability(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && assignment_target_root_is_aggregate_binding_for_buildability(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn direct_outer_aggregate_move_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    let Expr::Unary(unary) = unwrap_group_expr(expression) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) else {
        return false;
    };
    identifier_is_outer_aggregate_for_buildability(
        identifier,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )
}

#[derive(Clone, Copy)]
pub(super) enum ExplicitAggregateMoveScope<'a> {
    Any,
    OutsideLocals(&'a HashSet<String>),
}

pub(super) fn expression_explicit_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::Any,
    )
}

pub(super) fn expression_explicit_outer_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings),
    )
}

pub(super) fn explicit_aggregate_move_span_in_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) {
                explicit_aggregate_move_matches_identifier(
                    identifier,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
                .then_some(unary.span)
            } else {
                explicit_aggregate_move_span_in_expression(
                    &unary.operand,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            }
        }
        Expr::ArrayLiteral(literal) => literal.elements.iter().find_map(|element| {
            explicit_aggregate_move_span_in_expression(
                element,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::StructLiteral(literal) => literal.fields.iter().find_map(|field| {
            explicit_aggregate_move_span_in_expression(
                &field.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Propagate(propagation) => explicit_aggregate_move_span_in_expression(
            &propagation.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Force(force) => explicit_aggregate_move_span_in_expression(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Catch(catch) => explicit_aggregate_move_span_in_expression(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Borrow(borrow) => explicit_aggregate_move_span_in_expression(
            &borrow.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Unary(unary) => explicit_aggregate_move_span_in_expression(
            &unary.operand,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Binary(binary) => explicit_aggregate_move_span_in_expression(
            &binary.left,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &binary.right,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::TypeConversion(conversion) => explicit_aggregate_move_span_in_expression(
            &conversion.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Call(call) => explicit_aggregate_move_span_in_expression(
            &call.callee,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            call.arguments.iter().find_map(|argument| {
                explicit_aggregate_move_span_in_expression(
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Member(member) => explicit_aggregate_move_span_in_expression(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Index(index) => explicit_aggregate_move_span_in_expression(
            &index.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &index.index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Group(group) => explicit_aggregate_move_span_in_expression(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Otherwise(expression) => explicit_aggregate_move_span_in_expression(
            &expression.value,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &expression.fallback,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Match(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().find_map(|part| {
            if let InterpolatedStringPart::Expression(part) = part {
                explicit_aggregate_move_span_in_expression(
                    &part.expression,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            } else {
                None
            }
        }),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

pub(super) fn explicit_aggregate_move_span_in_block(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => block
            .statements
            .iter()
            .find_map(|statement| {
                explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
            .or_else(|| {
                block.result.as_ref().and_then(|result| {
                    explicit_aggregate_move_span_in_expression(
                        result,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        scope,
                    )
                })
            }),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut nested_locals = local_bindings.clone();
            for statement in &block.statements {
                let span = explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                );
                if span.is_some() {
                    return span;
                }
                if let Stmt::Binding(statement) = statement {
                    nested_locals.insert(statement.name.clone());
                }
            }
            block.result.as_ref().and_then(|result| {
                explicit_aggregate_move_span_in_expression(
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                )
            })
        }
    }
}

pub(super) fn explicit_aggregate_move_span_in_statement(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => None,
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            explicit_aggregate_move_span_in_expression(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Binding(statement) => explicit_aggregate_move_span_in_expression(
            &statement.initializer,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Assignment(statement) => explicit_aggregate_move_span_in_expression(
            &statement.target,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::Switch(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::ForRange(statement) => explicit_aggregate_move_span_in_expression(
            &statement.start,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.end,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            explicit_aggregate_move_span_in_for_range_body(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::While(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Loop(statement) => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Expression(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

pub(super) fn explicit_aggregate_move_span_in_for_range_body(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut body_locals = local_bindings.clone();
            body_locals.insert(statement.name.clone());
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&body_locals),
            )
        }
    }
}

pub(super) fn explicit_aggregate_move_span_in_payload_block(
    block: &Block,
    payload_name: Option<&str>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match (scope, payload_name) {
        (ExplicitAggregateMoveScope::OutsideLocals(local_bindings), Some(payload_name)) => {
            let mut nested_locals = local_bindings.clone();
            nested_locals.insert(payload_name.to_owned());
            explicit_aggregate_move_span_in_block(
                block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
            )
        }
        _ => explicit_aggregate_move_span_in_block(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

pub(super) fn explicit_aggregate_move_matches_identifier(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> bool {
    match scope {
        ExplicitAggregateMoveScope::Any => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            identifier_is_outer_aggregate_for_buildability(
                identifier,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                local_bindings,
            )
        }
    }
}

pub(super) fn assignment_target_root_is_aggregate_binding_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Member(member) => assignment_target_root_is_aggregate_binding_for_buildability(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(super) fn identifier_is_outer_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    !local_bindings.contains(&identifier.name)
        && identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

pub(super) fn identifier_is_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
        return false;
    };
    let Some(ty) = typecheck_facts.binding_type_expr(symbol.name_span) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    type_expr_is_supported_aggregate_value_for_sources(&ty, resolved, resolved_sources)
}

pub(super) fn statement_suffix_exits_function_for_buildability(
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    statement_sequence_or_result_exits_function_for_buildability(
        statements.get(index + 1..).unwrap_or(&[]),
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn statement_sequence_or_result_exits_function_for_buildability(
    statements: &[Stmt],
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop_for_buildability(statement) {
            return false;
        }
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ) {
            return true;
        }
    }
    result.is_some_and(|expression| {
        expression_exits_function_for_buildability(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(super) fn statement_exits_function_for_buildability(
    statement: &Stmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function_for_buildability(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Switch(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(super) fn if_statement_exits_function_for_buildability(
    statement: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn block_exits_function_for_buildability(
    block: &Block,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    statement_sequence_or_result_exits_function_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn expression_exits_function_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typecheck_facts, generic_substitutions),
            Some(ReturnShape::Never)
        ),
        Expr::If(statement) => if_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(statement) => if_is_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(statement) => switch_statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(super) fn statement_may_exit_current_loop_for_buildability(statement: &Stmt) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        Stmt::While(_) | Stmt::Loop(_) => false,
        _ => false,
    }
}

pub(super) fn block_may_exit_current_loop_for_buildability(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(statement_may_exit_current_loop_for_buildability)
        || block
            .result
            .as_deref()
            .is_some_and(expression_may_exit_current_loop_for_buildability)
}

pub(super) fn expression_may_exit_current_loop_for_buildability(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::IfIs(statement) => {
            block_may_exit_current_loop_for_buildability(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop_for_buildability)
        }
        Expr::Match(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop_for_buildability(&arm.body))
        }
        _ => false,
    }
}

pub(super) fn enqueue_member_replacement_drop_target(
    statement: &AssignmentStmt,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    if statement.operator != AssignmentOperator::Assign {
        return;
    }
    let Expr::Member(member) = unwrap_group_expr(&statement.target) else {
        return;
    };
    let Some(specialization) = typecheck_facts.field_drop_type_specialization(member.member_span)
    else {
        return;
    };
    let Some(specialization) = specialization.with_context_substitutions(generic_substitutions)
    else {
        return;
    };
    queue.push_back(call_target_for_source(
        specialization.declaration_span.source,
        root_source,
        specialization.target_name,
    ));
}

pub(super) fn expression_statement_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape_for_sources(
                call,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                Some(
                    ReturnShape::Void
                    | ReturnShape::Never
                    | ReturnShape::DiscardableScalar
                    | ReturnShape::DiscardableView
                    | ReturnShape::DiscardableAggregate,
                )
                | None => true,
                Some(ReturnShape::FallibleDiscardable | ReturnShape::Other) => false,
            }
        }
        Expr::Propagate(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(expression) => fallible_void_statement_inner_is_supported(
            &expression.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::StructLiteral(literal) => aggregate_literal_statement_is_supported(literal, resolved),
        _ => false,
    }
}

pub(super) fn catch_block_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return false;
    }

    let Some((last, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        catch_block_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && catch_block_terminal_statement_runtime_shape_is_buildable(
        last,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn catch_block_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) | Stmt::Binding(_) | Stmt::Assignment(_) => true,
        Stmt::Expression(statement) => expression_statement_is_supported(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Return(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

pub(super) fn catch_block_terminal_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => {
            expression_exits_function_for_buildability(
                &statement.expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
            ) || expression_statement_is_supported(
                &statement.expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

pub(super) fn otherwise_return_fallback_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return block.statements.iter().all(|statement| {
            otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        });
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && match terminal {
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_is_never_runtime_shape_is_buildable(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => false,
    }
}

pub(super) fn otherwise_binding_fallback_runtime_shape_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if block.result.is_some() {
        return block.statements.iter().all(|statement| {
            otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        });
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return false;
    };

    leading.iter().all(|statement| {
        otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && match terminal {
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Expression(statement) => expression_is_never_runtime_shape_is_buildable(
            &statement.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Drop(_) => false,
    }
}

pub(super) fn otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::Drop(_) => true,
        Stmt::Expression(statement) => expression_statement_is_supported(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Return(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

pub(super) fn otherwise_binding_fallback_leading_statement_runtime_shape_is_buildable(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    otherwise_return_fallback_leading_statement_runtime_shape_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn fallible_void_statement_inner_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => {
            match call_return_shape_for_sources(
                call,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                Some(ReturnShape::FallibleDiscardable) | None => true,
                Some(
                    ReturnShape::Void
                    | ReturnShape::Never
                    | ReturnShape::DiscardableScalar
                    | ReturnShape::DiscardableView
                    | ReturnShape::DiscardableAggregate
                    | ReturnShape::Other,
                ) => false,
            }
        }
        _ => false,
    }
}

pub(super) fn range_for_binding_type_is_buildable(
    statement: &ForRangeStmt,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.binding_scalar_view_kind(statement.name_span),
        Some(TypecheckScalarViewKind::I32 | TypecheckScalarViewKind::Usize)
    )
}

pub(super) fn assignment_operator_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator == AssignmentOperator::Assign {
        return true;
    }
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            matches!(
                typecheck_facts.binding_scalar_view_kind(symbol.name_span),
                Some(
                    TypecheckScalarViewKind::I32
                        | TypecheckScalarViewKind::Usize
                        | TypecheckScalarViewKind::U8
                )
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typecheck_facts)
        }
        Expr::Index(index) => {
            fixed_array_index_compound_assignment_is_buildable(
                index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) || slice_index_compound_assignment_is_buildable(
                &index.object,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => false,
    }
}

pub(super) fn slice_index_compound_assignment_is_buildable(
    object: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    matches!(
        slice_index_assignment_element_kind(
            object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions
        ),
        Some(
            TypecheckSliceElementKind::I32
                | TypecheckSliceElementKind::U8
                | TypecheckSliceElementKind::Usize,
        )
    )
}

pub(super) fn aggregate_field_compound_assignment_is_buildable(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.field_scalar_view_kind(member_span),
        Some(
            TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize,
        )
    )
}
