use super::*;

pub(in crate::driver::buildability) fn collect_void_effect_expression_diagnostics(
    expression: &Expr,
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
    match unwrap_group_expr(expression) {
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_expression_diagnostics(
                expression,
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
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_is_expression_diagnostics(
                expression,
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
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_match_expression_diagnostics(
                expression,
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
        _ => {
            if let Some(diagnostic) = unsupported_expression_statement_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                expression,
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
}

pub(in crate::driver::buildability) fn collect_void_effect_if_expression_diagnostics(
    expression: &crate::ast::IfStmt,
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
    collect_control_condition_move_diagnostics(
        &expression.condition,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
    collect_expression_diagnostics(
        &expression.condition,
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
    collect_void_effect_block_diagnostics(
        &expression.then_block,
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
    if let Some(else_block) = &expression.else_block {
        collect_void_effect_block_diagnostics(
            else_block,
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

pub(in crate::driver::buildability) fn collect_void_effect_if_is_expression_diagnostics(
    expression: &crate::ast::IfIsStmt,
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
    collect_payload_pattern_target_expression_diagnostics(
        &expression.expression,
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
    collect_void_effect_block_diagnostics(
        &expression.then_block,
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
    if let Some(else_block) = &expression.else_block {
        collect_void_effect_block_diagnostics(
            else_block,
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

pub(in crate::driver::buildability) fn collect_void_effect_match_expression_diagnostics(
    expression: &crate::ast::SwitchStmt,
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
    collect_payload_pattern_target_expression_diagnostics(
        &expression.expression,
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
    for arm in &expression.arms {
        collect_void_effect_block_diagnostics(
            &arm.body,
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
    if let Some(wildcard_arm) = &expression.wildcard_arm {
        collect_void_effect_block_diagnostics(
            &wildcard_arm.body,
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

pub(in crate::driver::buildability) fn collect_void_effect_block_diagnostics(
    block: &Block,
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
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            None,
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
    if let Some(result) = &block.result {
        collect_void_effect_expression_diagnostics(
            result,
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
