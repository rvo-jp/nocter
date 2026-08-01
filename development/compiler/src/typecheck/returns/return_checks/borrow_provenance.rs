use super::*;

pub(in crate::typecheck::returns) fn check_borrow_return_provenance(
    sources: &SourceMap,
    expression: &Expr,
    ty: &Type,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(provenance) = borrow_return_provenance_for_expression(
        expression,
        ty,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) else {
        return;
    };
    if let Some((region, description)) = provenance.first_region_origin(|_| true) {
        diagnostics.push(region_return_escape_diagnostic(
            sources,
            expression,
            description,
            region.declaration_span(),
            context,
        ));
        return;
    }
    if !type_contains_borrow_like(ty, resolved) {
        return;
    }
    let Some(source) = provenance.escaping_source() else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources, expression, source, context,
    ));
}

pub(in crate::typecheck::returns) fn check_propagated_fallible_error_borrow_return_provenance(
    sources: &SourceMap,
    expression: &PropagationExpr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    if !propagated_fallible_error_can_escape(
        &expression.expression,
        &context.declared_type,
        resolved,
        environment,
    ) {
        return;
    }

    let Some(provenance) = borrow_return_fallible_error_provenance_for_expression(
        &expression.expression,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) else {
        return;
    };
    if let Some((region, description)) = provenance.first_region_origin(|_| true) {
        diagnostics.push(region_return_escape_diagnostic(
            sources,
            &expression.expression,
            description,
            region.declaration_span(),
            context,
        ));
        return;
    }
    let Some(source) = provenance.escaping_source() else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources,
        &expression.expression,
        source,
        context,
    ));
}
