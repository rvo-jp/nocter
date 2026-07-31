use super::*;

pub(in crate::driver::buildability) fn collect_otherwise_return_expression_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_binding_initializer_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_assignment_value_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_scalar_view_value_expression_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_aggregate_value_expression_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_runtime_value_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_return_fallback_block_diagnostics(
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

pub(in crate::driver::buildability) fn collect_otherwise_value_fallback_block_diagnostics(
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
