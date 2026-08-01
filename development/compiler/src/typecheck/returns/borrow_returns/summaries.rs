use super::*;

pub(in crate::typecheck) fn callable_provenance_summaries(
    summary_sources: &[TypecheckSource<'_>],
) -> CallableProvenanceSummaries {
    let mut summaries = CallableProvenanceSummaries::default();
    for _ in 0..=borrow_return_callable_count(summary_sources) {
        let next = collect_callable_provenance_summaries(summary_sources, &summaries);
        if next == summaries {
            summaries = next;
            break;
        }
        summaries = next;
    }
    crate::typecheck::allocation::infer_callable_allocation_effects(
        summary_sources,
        &mut summaries,
    );
    summaries
}

pub(in crate::typecheck::returns) fn borrow_return_callable_count(
    summary_sources: &[TypecheckSource<'_>],
) -> usize {
    summary_sources
        .iter()
        .map(|source| {
            source
                .ast
                .items
                .iter()
                .map(item_callable_count)
                .sum::<usize>()
        })
        .sum()
}

pub(in crate::typecheck::returns) fn item_callable_count(item: &Item) -> usize {
    match item {
        Item::Function(_) => 1,
        Item::Impl(impl_) => impl_
            .members
            .iter()
            .filter(|member| matches!(member, ImplMember::Method(method) if method.body.is_some()))
            .count(),
        _ => 0,
    }
}

pub(in crate::typecheck::returns) fn collect_callable_provenance_summaries(
    summary_sources: &[TypecheckSource<'_>],
    previous: &CallableProvenanceSummaries,
) -> CallableProvenanceSummaries {
    let mut summaries = CallableProvenanceSummaries::default();
    for source in summary_sources {
        for item in &source.ast.items {
            match item {
                Item::Function(function) => {
                    let environment = environment_for_function(function, source.resolved);
                    let return_type = type_expr_to_type_in_environment(
                        &function.return_type,
                        source.resolved,
                        &environment,
                    );
                    let provenance = borrow_return_provenance_for_callable_body(
                        &function.body,
                        &return_type,
                        source.resolved,
                        &environment,
                        previous,
                    )
                    .unwrap_or(ValueProvenance::Independent);
                    summaries.insert_result(
                        CallableId::declared_at(function_summary_key(function)),
                        provenance,
                    );
                }
                Item::Impl(impl_) => {
                    for member in &impl_.members {
                        let ImplMember::Method(method) = member else {
                            continue;
                        };
                        let Some(body) = &method.body else {
                            continue;
                        };
                        let environment = environment_for_method(method, source.resolved, impl_);
                        let return_type = type_expr_to_type_in_environment(
                            &method.return_type,
                            source.resolved,
                            &environment,
                        );
                        let provenance = borrow_return_provenance_for_callable_body(
                            body,
                            &return_type,
                            source.resolved,
                            &environment,
                            previous,
                        )
                        .unwrap_or(ValueProvenance::Independent);
                        summaries
                            .insert_result(CallableId::declared_at(method.name_span), provenance);
                    }
                }
                _ => {}
            }
        }
    }
    summaries
}

pub(in crate::typecheck::returns) fn function_summary_key(
    function: &crate::ast::FunctionDecl,
) -> ByteSpan {
    if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    }
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_callable_body(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let mut flow = ProvenanceFlow::default();
    let mut body_environment = environment.clone();
    let mut body_borrow_provenance = ProvenanceEnvironment::default();
    collect_return_statement_provenance(
        block,
        return_type,
        resolved,
        &mut body_environment,
        &mut body_borrow_provenance,
        summaries,
        &mut flow,
    );
    collect_block_result_provenance(
        block,
        return_type,
        resolved,
        environment,
        &ProvenanceEnvironment::default(),
        summaries,
        &mut flow,
    );
    flow.into_return_provenance(return_type)
}
