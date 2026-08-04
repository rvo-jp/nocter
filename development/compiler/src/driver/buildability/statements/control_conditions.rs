use super::*;

pub(in crate::driver::buildability) fn collect_control_condition_move_diagnostics(
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

    diagnostics.push(unsupported_native_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "select the branch before moving aggregate values until control-flow condition move lowering is promoted",
    ));
}

pub(in crate::driver::buildability) fn collect_nonterminal_control_block_aggregate_diagnostics(
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

pub(in crate::driver::buildability) fn collect_nonterminal_control_payload_block_aggregate_diagnostics(
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

pub(in crate::driver::buildability) fn collect_nonterminal_control_block_aggregate_diagnostics_with_locals(
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
                    diagnostics.push(unsupported_native_build_diagnostic(
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
                    diagnostics.push(unsupported_native_build_diagnostic(
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
                    diagnostics.push(unsupported_native_build_diagnostic(
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
                diagnostics.push(unsupported_native_build_diagnostic(
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
        diagnostics.push(unsupported_native_build_diagnostic(
            sources,
            span,
            "explicit outer aggregate moves inside non-terminal control-flow results",
            "move values created inside the branch/body, or move outer values only before a statement that immediately exits the function until broader control-flow move lowering is promoted",
        ));
    }
}
