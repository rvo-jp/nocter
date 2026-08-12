use super::*;

pub(in crate::driver::buildability) fn collect_payload_pattern_target_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Otherwise(otherwise) = unwrap_group_expr(expression)
        && let Some(expected_type) = typed_hir.expression_type_expr(expression.span())
    {
        let expected_type = substitute_type_expr_parameters(expected_type, generic_substitutions);
        collect_otherwise_aggregate_value_expression_diagnostics(
            otherwise,
            &expected_type,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
        return;
    }

    collect_expression_diagnostics(
        expression,
        sources,
        resolved,
        typed_hir,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

pub(in crate::driver::buildability) fn collect_value_expression_diagnostics(
    expression: &Expr,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match unwrap_group_expr(expression) {
        Expr::If(expression) if value_if_expression_is_buildable(expression) => {
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typed_hir,
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
        Expr::IfIs(expression)
            if value_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) =>
        {
            collect_payload_pattern_target_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_value_block_diagnostics(
                &expression.then_block,
                return_type,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
                    return_type,
                    sources,
                    resolved,
                    typed_hir,
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
        Expr::Match(expression)
            if value_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) =>
        {
            collect_payload_pattern_target_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_value_block_diagnostics(
                    &arm.body,
                    return_type,
                    sources,
                    resolved,
                    typed_hir,
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
                collect_value_block_diagnostics(
                    &wildcard_arm.body,
                    return_type,
                    sources,
                    resolved,
                    typed_hir,
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
        Expr::Otherwise(expression) => {
            collect_otherwise_scalar_view_value_expression_diagnostics(
                expression,
                return_type,
                sources,
                resolved,
                typed_hir,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        _ => collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
    }
}

pub(in crate::driver::buildability) fn collect_value_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    let Some(result) = &block.result else {
        return;
    };
    collect_value_expression_diagnostics(
        result,
        return_type,
        sources,
        resolved,
        typed_hir,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}
