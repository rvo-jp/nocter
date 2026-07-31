use super::*;

pub(super) fn collect_callable_diagnostics(
    callable: &IndexedCallable<'_>,
    sources: &SourceMap,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for issue in &callable.issues {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            issue.span,
            issue.construct,
            issue.help,
        ));
    }

    enqueue_drop_targets_in_callable(callable, root_source, queue);

    collect_terminal_return_block_diagnostics(
        callable.body,
        callable.return_type.as_ref(),
        sources,
        callable.resolved,
        callable.typecheck_facts,
        &callable.substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

pub(super) fn enqueue_drop_targets_in_callable(
    callable: &IndexedCallable<'_>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    for specialization in callable.typecheck_facts.drop_type_specializations() {
        if !span_contains(callable.span, specialization.self_ty.span()) {
            continue;
        }
        let Some(specialization) =
            specialization.with_context_substitutions(&callable.substitutions)
        else {
            continue;
        };
        queue.push_back(call_target_for_source(
            specialization.declaration_span.source,
            root_source,
            specialization.target_name,
        ));
    }
}

pub(super) fn collect_terminal_return_block_diagnostics(
    block: &Block,
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
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
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
    if let Some(result) = result {
        collect_terminal_return_expression_diagnostics(
            result,
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

pub(super) fn collect_block_diagnostics(
    block: &Block,
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
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typecheck_facts,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
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
    if let Some(result) = result {
        collect_expression_diagnostics(
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

pub(super) fn reachable_block_parts_for_buildability<'a>(
    statements: &'a [Stmt],
    result: Option<&'a Expr>,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> (&'a [Stmt], Option<&'a Expr>) {
    for (index, statement) in statements.iter().enumerate() {
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ) {
            return (&statements[..=index], None);
        }
    }

    (statements, result)
}

pub(super) fn collect_terminal_return_expression_diagnostics(
    expression: &Expr,
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
    match unwrap_group_expr(expression) {
        Expr::Otherwise(expression) => {
            collect_otherwise_return_expression_diagnostics(
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
        Expr::If(expression) if terminal_if_expression_is_buildable(expression) => {
            collect_terminal_control_condition_move_diagnostics(
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
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
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
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
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
        Expr::IfIs(expression)
            if terminal_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
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
            collect_terminal_return_block_diagnostics(
                &expression.then_block,
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
            if let Some(else_block) = &expression.else_block {
                collect_terminal_return_block_diagnostics(
                    else_block,
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
        Expr::Match(expression)
            if terminal_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
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
                collect_terminal_return_block_diagnostics(
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
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_terminal_return_block_diagnostics(
                    &wildcard_arm.body,
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
        _ => collect_expression_diagnostics(
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
        ),
    }
}

pub(super) fn collect_value_expression_diagnostics(
    expression: &Expr,
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
    match unwrap_group_expr(expression) {
        Expr::If(expression) if value_if_expression_is_buildable(expression) => {
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
            collect_value_block_diagnostics(
                &expression.then_block,
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
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
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
        Expr::IfIs(expression)
            if value_if_is_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
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
            collect_value_block_diagnostics(
                &expression.then_block,
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
            if let Some(else_block) = &expression.else_block {
                collect_value_block_diagnostics(
                    else_block,
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
        Expr::Match(expression)
            if value_match_expression_is_buildable(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) =>
        {
            collect_expression_diagnostics(
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
                collect_value_block_diagnostics(
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
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_value_block_diagnostics(
                    &wildcard_arm.body,
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
        Expr::Otherwise(expression) => {
            collect_otherwise_scalar_view_value_expression_diagnostics(
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
        _ => collect_expression_diagnostics(
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
        ),
    }
}

pub(super) fn collect_value_block_diagnostics(
    block: &Block,
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
    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
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
    let Some(result) = &block.result else {
        return;
    };
    collect_value_expression_diagnostics(
        result,
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

pub(super) fn collect_otherwise_return_expression_diagnostics(
    expression: &OtherwiseExpr,
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
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 return subset",
            "end runtime-shipped `otherwise` return fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
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
    collect_otherwise_return_fallback_block_diagnostics(
        &expression.fallback,
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

pub(super) fn collect_otherwise_binding_initializer_diagnostics(
    expression: &OtherwiseExpr,
    binding_is_scalar_or_view: bool,
    binding_fixed_array_type: Option<&TypeExpr>,
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
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_binding_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 binding subset",
            "end runtime-shipped `otherwise` binding fallbacks with a value, direct `return`, loop-local `break`/`continue`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
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
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        binding_fixed_array_type,
        binding_is_scalar_or_view,
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

pub(super) fn collect_otherwise_assignment_value_diagnostics(
    expression: &OtherwiseExpr,
    assignment_aggregate_type: Option<&TypeExpr>,
    assignment_is_scalar_or_view: bool,
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
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 assignment subset",
            "end runtime-shipped `otherwise` assignment fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
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
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        assignment_aggregate_type,
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
}

pub(super) fn collect_otherwise_scalar_view_value_expression_diagnostics(
    expression: &OtherwiseExpr,
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
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 scalar/view value subset",
            "end runtime-shipped scalar/view `otherwise` value fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
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
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        None,
        true,
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

pub(super) fn collect_otherwise_aggregate_value_expression_diagnostics(
    expression: &OtherwiseExpr,
    expected_type: &TypeExpr,
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
    collect_otherwise_runtime_value_diagnostics(
        expression,
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
        diagnostics,
    );
    if !otherwise_return_fallback_runtime_shape_is_buildable(
        &expression.fallback,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        diagnostics.push(unsupported_v0_build_diagnostic(
            sources,
            expression.fallback.span,
            "`otherwise` fallback blocks outside the v0 aggregate value subset",
            "end runtime-shipped aggregate `otherwise` fallbacks with a value, direct `return`, or supported `never` expression until broader fallback lowering is promoted",
        ));
    }

    collect_expression_diagnostics(
        &expression.value,
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
    collect_otherwise_value_fallback_block_diagnostics(
        &expression.fallback,
        Some(expected_type),
        false,
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

pub(super) fn collect_otherwise_runtime_value_diagnostics(
    expression: &OtherwiseExpr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if otherwise_optional_value_call_is_buildable(
        &expression.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) {
        return;
    }

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        expression.value.span(),
        "`otherwise` values outside the v0 runtime subset",
        "apply runtime-shipped `otherwise` directly to a call returning a top-level optional value",
    ));
}

pub(super) fn collect_otherwise_return_fallback_block_diagnostics(
    block: &Block,
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
    if block.result.is_none() {
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
        return;
    }

    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
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
    if let Some(result) = &block.result {
        collect_terminal_return_expression_diagnostics(
            result,
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

pub(super) fn collect_otherwise_value_fallback_block_diagnostics(
    block: &Block,
    expected_aggregate_type: Option<&TypeExpr>,
    result_is_scalar_or_view: bool,
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
    if block.result.is_none() {
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
        return;
    }

    for statement in &block.statements {
        collect_statement_diagnostics(
            statement,
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
    if let Some(result) = &block.result {
        if fixed_array_literal_for_type_has_fixed_array_type(
            result,
            expected_aggregate_type,
            resolved,
            resolved_sources,
        ) {
            collect_fixed_array_literal_elements_diagnostics(
                unwrap_group_expr(result),
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
        } else if result_is_scalar_or_view {
            collect_value_expression_diagnostics(
                result,
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
}

pub(super) fn collect_void_effect_expression_diagnostics(
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

pub(super) fn collect_void_effect_if_expression_diagnostics(
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

pub(super) fn collect_void_effect_if_is_expression_diagnostics(
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
    collect_expression_diagnostics(
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

pub(super) fn collect_void_effect_match_expression_diagnostics(
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
    collect_expression_diagnostics(
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

pub(super) fn collect_void_effect_block_diagnostics(
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

pub(super) fn binding_initializer_may_use_value_control_expression(
    statement: &crate::ast::BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let ty = statement.ty.clone().or_else(|| {
        typecheck_facts
            .binding_type_expr(statement.name_span)
            .cloned()
    });
    let Some(ty) = ty else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
}

pub(super) fn assignment_value_may_use_value_control_expression(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            typecheck_facts
                .binding_type_expr(symbol.name_span)
                .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
                .is_some_and(|ty| {
                    type_expr_is_buildable_scalar_or_view_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
                })
        }
        Expr::Member(member) => typecheck_facts
            .field_scalar_view_kind(member.member_span)
            .is_some_and(field_kind_may_use_value_control_expression),
        _ => false,
    }
}

pub(super) fn call_argument_may_use_value_control_expression(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
    .is_some_and(|ty| {
        type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
    })
}

pub(super) fn otherwise_aggregate_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(argument) else {
        return None;
    };
    let ty = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(super) fn otherwise_aggregate_struct_field_type(
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(&field.value) else {
        return None;
    };
    let ty = field_type_expr_for_span(field.name_span, resolved, typecheck_facts)?;
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(super) fn otherwise_aggregate_member_root_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let otherwise = aggregate_member_root_otherwise(&member.object)?;
    let Expr::Call(call) = unwrap_group_expr(&otherwise.value) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let ty = type_expr_top_level_optional_success_with_resolver(
        &return_type,
        resolved,
        &source_resolver,
    )?;
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(super) fn aggregate_member_root_otherwise(expression: &Expr) -> Option<&OtherwiseExpr> {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(otherwise) => Some(otherwise),
        Expr::Member(member) => aggregate_member_root_otherwise(&member.object),
        _ => None,
    }
}

pub(super) fn call_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref()
        && let Some(ty) = method_call_argument_parameter_type(
            member,
            index,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    {
        return Some(ty);
    }

    let signature = resolved.call_signature_for_call(call)?;
    let parameter = signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
    }

    ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    Some(ty)
}

pub(super) fn method_call_argument_parameter_type(
    member: &crate::ast::MemberExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let method_name_span = typecheck_facts.method_call_target(member.member_span)?;
    let method = resolved.method_signature_by_name_span(method_name_span)?;
    let parameter = method.signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
    {
        let self_substitution =
            HashMap::from([("Self".to_string(), specialization.self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
        return Some(substitute_type_expr_parameters(&ty, generic_substitutions));
    }

    if typecheck_facts
        .generic_method_call_target(member.member_span)
        .is_some()
    {
        return None;
    }

    if let Some(self_ty) = &method.impl_target_ty {
        let self_substitution = HashMap::from([("Self".to_string(), self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
    }
    Some(substitute_type_expr_parameters(&ty, generic_substitutions))
}

pub(super) fn struct_literal_field_may_use_value_control_expression(
    field_name_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    typecheck_facts
        .field_scalar_view_kind(field_name_span)
        .is_some_and(field_kind_may_use_value_control_expression)
}

pub(super) fn field_kind_may_use_value_control_expression(kind: TypecheckScalarViewKind) -> bool {
    match kind {
        TypecheckScalarViewKind::I32
        | TypecheckScalarViewKind::U8
        | TypecheckScalarViewKind::Usize
        | TypecheckScalarViewKind::Bool
        | TypecheckScalarViewKind::Str => true,
        TypecheckScalarViewKind::Slice(element) => {
            typecheck_slice_element_kind_is_buildable(element)
        }
    }
}
