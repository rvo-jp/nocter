use super::*;

pub(in crate::driver::buildability) fn collect_callable_diagnostics(
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

pub(in crate::driver::buildability) fn enqueue_drop_targets_in_callable(
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

pub(in crate::driver::buildability) fn collect_terminal_return_block_diagnostics(
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

pub(in crate::driver::buildability) fn collect_block_diagnostics(
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

pub(in crate::driver::buildability) fn reachable_block_parts_for_buildability<'a>(
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
