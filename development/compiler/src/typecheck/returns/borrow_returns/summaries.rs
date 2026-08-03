use super::*;
use crate::ast::{ResultProvenanceClause, ResultProvenanceOriginKind};

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
        Item::Primitive(_) => 1,
        Item::Interface(interface) => interface.methods.len(),
        Item::Literal(_) => 1,
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
                    let declared = function.result_provenance.as_ref().and_then(|clause| {
                        result_provenance_contract(
                            clause,
                            None,
                            &function.parameters.parameters,
                            source.resolved,
                        )
                        .ok()
                    });
                    let callable = CallableId::declared_at(function_summary_key(function));
                    summaries.insert_result(callable, declared.unwrap_or(provenance));
                    seed_declared_allocation_effect(
                        &mut summaries,
                        callable,
                        function.result_provenance.as_ref(),
                    );
                }
                Item::Primitive(primitive) => {
                    let Some(provenance) =
                        primitive.result_provenance.as_ref().and_then(|clause| {
                            result_provenance_contract(
                                clause,
                                None,
                                &primitive.parameters.parameters,
                                source.resolved,
                            )
                            .ok()
                        })
                    else {
                        continue;
                    };
                    let callable = CallableId::declared_at(primitive.name_span);
                    summaries.insert_result(callable, provenance);
                    seed_declared_allocation_effect(
                        &mut summaries,
                        callable,
                        primitive.result_provenance.as_ref(),
                    );
                }
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        let environment =
                            environment_for_interface_method(method, source.resolved, interface);
                        let declared = method.result_provenance.as_ref().and_then(|clause| {
                            result_provenance_contract(
                                clause,
                                Some(method),
                                &method.parameters.parameters,
                                source.resolved,
                            )
                            .ok()
                        });
                        let inferred = method.body.as_ref().and_then(|body| {
                            let return_type = type_expr_to_type_in_environment(
                                &method.return_type,
                                source.resolved,
                                &environment,
                            );
                            borrow_return_provenance_for_callable_body(
                                body,
                                &return_type,
                                source.resolved,
                                &environment,
                                previous,
                            )
                        });
                        let provenance = declared.or(inferred).or_else(|| {
                            elided_result_provenance_contract(
                                Some(method),
                                &method.parameters.parameters,
                                &method.return_type,
                                source.resolved,
                            )
                        });
                        let Some(provenance) = provenance else {
                            continue;
                        };
                        let callable = CallableId::declared_at(method.name_span);
                        summaries.insert_result(callable, provenance);
                        seed_declared_allocation_effect(
                            &mut summaries,
                            callable,
                            method.result_provenance.as_ref(),
                        );
                    }
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
                        let declared = method.result_provenance.as_ref().and_then(|clause| {
                            result_provenance_contract(
                                clause,
                                Some(method),
                                &method.parameters.parameters,
                                source.resolved,
                            )
                            .ok()
                        });
                        let callable = CallableId::declared_at(method.name_span);
                        summaries.insert_result(callable, declared.unwrap_or(provenance));
                        seed_declared_allocation_effect(
                            &mut summaries,
                            callable,
                            method.result_provenance.as_ref(),
                        );
                    }
                }
                Item::Literal(literal) => {
                    let environment = environment_for_literal(literal, source.resolved);
                    let return_type = type_expr_to_type_in_environment(
                        &literal.return_type,
                        source.resolved,
                        &environment,
                    );
                    let provenance = borrow_return_provenance_for_callable_body(
                        &literal.body,
                        &return_type,
                        source.resolved,
                        &environment,
                        previous,
                    )
                    .unwrap_or(ValueProvenance::Independent);
                    let declared = literal.result_provenance.as_ref().and_then(|clause| {
                        result_provenance_contract(
                            clause,
                            None,
                            &literal.parameters.parameters,
                            source.resolved,
                        )
                        .ok()
                    });
                    let callable = CallableId::declared_at(literal.span);
                    summaries.insert_result(callable, declared.unwrap_or(provenance));
                    seed_declared_allocation_effect(
                        &mut summaries,
                        callable,
                        literal.result_provenance.as_ref(),
                    );
                }
                _ => {}
            }
        }
    }
    summaries
}

fn seed_declared_allocation_effect(
    summaries: &mut CallableProvenanceSummaries,
    callable: CallableId,
    clause: Option<&ResultProvenanceClause>,
) {
    if clause.is_some_and(|clause| {
        clause.origins.iter().any(|origin| {
            matches!(
                origin.kind,
                ResultProvenanceOriginKind::CurrentAllocationContext
            )
        })
    }) {
        summaries.set_needs_current_allocation_context(callable);
    }
}

pub(in crate::typecheck) fn function_summary_key(function: &crate::ast::FunctionDecl) -> ByteSpan {
    if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    }
}

pub(in crate::typecheck) fn borrow_return_provenance_for_callable_body(
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
