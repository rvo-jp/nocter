use super::*;

pub(in crate::driver::buildability) fn collect_terminal_return_expression_diagnostics(
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
        Expr::Otherwise(expression) => {
            collect_otherwise_return_expression_diagnostics(
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
        Expr::ArrayLiteral(_)
            if fixed_array_literal_return_has_fixed_array_type(
                expression,
                return_type,
                resolved,
                resolved_sources,
            ) =>
        {
            collect_fixed_array_literal_elements_diagnostics(
                unwrap_group_expr(expression),
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
        Expr::If(expression)
            if void_effect_if_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_expression_diagnostics(
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
        Expr::IfIs(expression)
            if void_effect_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_if_is_expression_diagnostics(
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
        Expr::Match(expression)
            if void_effect_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) =>
        {
            collect_void_effect_match_expression_diagnostics(
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
        Expr::If(expression) if terminal_if_expression_is_buildable(expression) => {
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
            collect_terminal_return_block_diagnostics(
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
                collect_terminal_return_block_diagnostics(
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
            if terminal_if_is_expression_is_buildable(
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
            collect_terminal_return_block_diagnostics(
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
                collect_terminal_return_block_diagnostics(
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
            if terminal_match_expression_is_buildable(
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
                collect_terminal_return_block_diagnostics(
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
                collect_terminal_return_block_diagnostics(
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
