use super::*;

pub(in crate::typecheck::returns) fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    if context.success_type().first_unsized_part().is_some() {
        return;
    }

    let block_exits = check_block_return_statements(
        sources,
        block,
        context,
        resolved,
        diagnostics,
        environment,
        borrow_provenance,
        summaries,
    );

    if block_exits {
        return;
    }

    if let Some(result) = &block.result {
        check_body_result_return(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return;
    }

    if context.requires_explicit_return()
        && !block_guarantees_return_or_never(block, resolved, environment)
    {
        diagnostics.push(missing_return_diagnostic(sources, block.span, context));
    }
}

pub(in crate::typecheck::returns) fn check_block_return_statements(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> bool {
    for statement in &block.statements {
        check_statement_returns(
            sources,
            statement,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return true;
        }
    }
    if let Some(result) = &block.result {
        check_expression_for_nested_returns(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return expression_type(result, resolved, environment) == Type::Never;
    }

    false
}
