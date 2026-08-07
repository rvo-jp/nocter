use super::*;
use crate::ast::ResultAllocationModifier;

pub(in crate::typecheck) fn callable_provenance_summaries(
    summary_sources: &[TypecheckSource<'_>],
) -> CallableProvenanceSummaries {
    // Body-backed callables start at the semantic bottom. This makes a call
    // inside a recursive SCC consume inferred evidence from the previous
    // iteration instead of falling back to its written `alloc` upper bound.
    // Bodyless declarations are intentionally absent and continue to use
    // their written contract at call sites.
    let floor = body_backed_summary_floor(summary_sources);
    let mut summaries = floor.clone();
    for _ in 0..=borrow_return_callable_count(summary_sources) {
        let mut next = collect_callable_provenance_summaries(summary_sources, &summaries);
        next.merge_missing_from(&floor);
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

fn body_backed_summary_floor(
    summary_sources: &[TypecheckSource<'_>],
) -> CallableProvenanceSummaries {
    let mut summaries = CallableProvenanceSummaries::default();
    for source in summary_sources {
        for item in &source.ast.items {
            match item {
                Item::Function(function) => summaries.insert_result(
                    CallableId::declared_at(function_summary_key(function)),
                    ValueProvenance::Independent,
                ),
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        if method.body.is_some() {
                            summaries.insert_result(
                                CallableId::declared_at(method.name_span),
                                ValueProvenance::Independent,
                            );
                        }
                    }
                }
                Item::Impl(impl_) => {
                    for member in &impl_.members {
                        if let ImplMember::Method(method) = member
                            && method.body.is_some()
                        {
                            summaries.insert_result(
                                CallableId::declared_at(method.name_span),
                                ValueProvenance::Independent,
                            );
                        }
                    }
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        summaries.insert_result(
                            CallableId::declared_at(function_summary_key(function)),
                            ValueProvenance::Independent,
                        );
                    }
                    for (_, literal) in construct.literals() {
                        summaries.insert_result(
                            CallableId::declared_at(literal.span),
                            ValueProvenance::Independent,
                        );
                    }
                }
                Item::Primitive(_)
                | Item::Test(_)
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_) => {}
            }
        }
    }
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
        Item::Construct(construct) => construct.functions().count() + construct.literals().count(),
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
                    summaries.insert_result(
                        callable,
                        result_with_declared_origins(provenance, declared),
                    );
                    collect_retained_input_mutations(
                        &function.body,
                        None,
                        &function.parameters.parameters,
                        source.resolved,
                        &environment,
                        previous,
                        &mut summaries,
                        callable,
                    );
                }
                Item::Primitive(primitive) => {
                    let declared = primitive.result_provenance.as_ref().and_then(|clause| {
                        result_provenance_contract(
                            clause,
                            None,
                            &primitive.parameters.parameters,
                            source.resolved,
                        )
                        .ok()
                    });
                    if declared.is_none() && primitive.result_allocation.is_none() {
                        continue;
                    }
                    let provenance = declared_result_allocation(
                        declared.unwrap_or(ValueProvenance::Independent),
                        primitive.result_allocation.as_ref(),
                    );
                    let callable = CallableId::declared_at(primitive.name_span);
                    summaries.insert_result(callable, provenance);
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
                        let provenance = match inferred {
                            Some(inferred) => {
                                Some(result_with_declared_origins(inferred, declared))
                            }
                            None => declared.or_else(|| {
                                elided_result_provenance_contract(
                                    Some(method),
                                    &method.parameters.parameters,
                                    &method.return_type,
                                    source.resolved,
                                )
                            }),
                        };
                        let provenance =
                            if method.body.is_none() && method.result_allocation.is_some() {
                                Some(declared_result_allocation(
                                    provenance.unwrap_or(ValueProvenance::Independent),
                                    method.result_allocation.as_ref(),
                                ))
                            } else {
                                provenance
                            };
                        let Some(provenance) = provenance else {
                            continue;
                        };
                        let callable = CallableId::declared_at(method.name_span);
                        summaries.insert_result(callable, provenance);
                        if let Some(body) = &method.body {
                            collect_retained_input_mutations(
                                body,
                                Some(&method.receiver),
                                &method.parameters.parameters,
                                source.resolved,
                                &environment,
                                previous,
                                &mut summaries,
                                callable,
                            );
                        }
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
                        summaries.insert_result(
                            callable,
                            result_with_declared_origins(provenance, declared),
                        );
                        collect_retained_input_mutations(
                            body,
                            Some(&method.receiver),
                            &method.parameters.parameters,
                            source.resolved,
                            &environment,
                            previous,
                            &mut summaries,
                            callable,
                        );
                    }
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
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
                        summaries.insert_result(
                            callable,
                            result_with_declared_origins(provenance, declared),
                        );
                        collect_retained_input_mutations(
                            &function.body,
                            None,
                            &function.parameters.parameters,
                            source.resolved,
                            &environment,
                            previous,
                            &mut summaries,
                            callable,
                        );
                    }
                    for (_, literal) in construct.literals() {
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
                        summaries.insert_result(
                            callable,
                            result_with_declared_origins(provenance, declared),
                        );
                        collect_retained_input_mutations(
                            &literal.body,
                            None,
                            &literal.parameters.parameters,
                            source.resolved,
                            &environment,
                            previous,
                            &mut summaries,
                            callable,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    summaries
}

fn result_with_declared_origins(
    inferred: ValueProvenance,
    declared: Option<ValueProvenance>,
) -> ValueProvenance {
    let Some(mut declared) = declared else {
        return inferred;
    };
    declared.merge(&inferred.retain_only_result_allocations());
    declared
}

fn declared_result_allocation(
    provenance: ValueProvenance,
    modifier: Option<&ResultAllocationModifier>,
) -> ValueProvenance {
    if modifier.is_some() {
        provenance.returned_allocation()
    } else {
        provenance
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
    let provenance = flow.into_return_provenance(return_type);
    if type_may_carry_result_provenance(return_type, resolved) {
        provenance
    } else {
        provenance.map(ValueProvenance::without_result_allocation)
    }
}
