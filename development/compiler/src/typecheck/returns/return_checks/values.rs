use super::*;

pub(in crate::typecheck::returns) fn check_body_result_return(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let expected = context.success_type();
    let actual = crate::typecheck::literals::literal_expression_type_with_expected(
        expression,
        Some(expected),
        resolved,
        environment,
    );

    if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
        return;
    }

    if expected == &Type::Void {
        if actual == Type::Void
            || actual == Type::Never
            || return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            )
        {
            return;
        }

        diagnostics.push(unexpected_body_result_diagnostic(
            sources, expression, context,
        ));
        return;
    }

    if expected.first_unsized_part().is_some() {
        return;
    }

    if return_expression_is_fallible_failure(expression, &actual, context, resolved, environment) {
        check_borrow_return_provenance(
            sources,
            expression,
            &actual,
            context,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            diagnostics,
        );
        return;
    }

    if actual != context.declared_type
        && !is_expression_assignable(expected, expression, resolved, environment)
    {
        diagnostics.push(body_result_type_mismatch_diagnostic(
            sources, expression, expected, &actual, context,
        ));
        return;
    }

    check_borrow_return_provenance(
        sources,
        expression,
        &actual,
        context,
        resolved,
        environment,
        borrow_provenance,
        summaries,
        diagnostics,
    );

    if let Some(source) = implicit_non_copy_owned_value_source(expression, resolved, environment) {
        diagnostics.push(non_copy_struct_return_diagnostic(
            sources,
            expression,
            &source.source_name,
            &source.type_name,
            source.kind,
            context,
        ));
    }
}

pub(in crate::typecheck::returns) fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let expected = context.success_type();
    if expected == &Type::Never {
        diagnostics.push(never_return_statement_diagnostic(
            sources, statement, context,
        ));
        return;
    }

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Unresolved(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            let actual = crate::typecheck::literals::literal_expression_type_with_expected(
                expression,
                Some(expected),
                resolved,
                environment,
            );
            if actual == Type::Never {
                return;
            }
            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                check_borrow_return_provenance(
                    sources,
                    expression,
                    &actual,
                    context,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    diagnostics,
                );
                return;
            }

            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = crate::typecheck::literals::literal_expression_type_with_expected(
                expression,
                Some(expected),
                resolved,
                environment,
            );
            if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
                return;
            }
            if expected.first_unsized_part().is_some() {
                return;
            }

            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                check_borrow_return_provenance(
                    sources,
                    expression,
                    &actual,
                    context,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    diagnostics,
                );
                return;
            }

            if actual != context.declared_type
                && !is_expression_assignable(expected, expression, resolved, environment)
            {
                diagnostics.push(return_type_mismatch_diagnostic(
                    sources, expression, expected, &actual, context,
                ));
                return;
            }

            check_borrow_return_provenance(
                sources,
                expression,
                &actual,
                context,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                diagnostics,
            );

            if let Some(source) =
                implicit_non_copy_owned_value_source(expression, resolved, environment)
            {
                diagnostics.push(non_copy_struct_return_diagnostic(
                    sources,
                    expression,
                    &source.source_name,
                    &source.type_name,
                    source.kind,
                    context,
                ));
            }
        }
    }
}
