use super::*;

pub(in crate::typecheck) fn callable_provenance_summaries(
    summary_sources: &[TypecheckSource<'_>],
) -> CallableProvenanceSummaries {
    // Body-backed callables start at the semantic bottom so recursive SCCs
    // consume only prior body evidence. Bodyless abstract declarations are
    // seeded below from their external `from` contract or a type-directed
    // conservative result-storage capability.
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
                Item::Coerce(coerce) => {
                    let impl_ = coerce.callable_impl();
                    for member in &impl_.members {
                        if let ImplMember::Method(method) = member {
                            summaries.insert_result(
                                CallableId::declared_at(method.name_span),
                                ValueProvenance::Independent,
                            );
                        }
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
        Item::Coerce(coerce) => coerce.entries.len(),
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
                            ResultProvenanceInputs::parameters(&function.parameters.parameters),
                            source.resolved,
                        )
                        .ok()
                    });
                    let callable = CallableId::declared_at(function_summary_key(function));
                    summaries.insert_result(
                        callable,
                        result_with_declared_fallback(
                            provenance,
                            declared,
                            &return_type,
                            source.resolved,
                        ),
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
                            ResultProvenanceInputs::parameters(&primitive.parameters.parameters),
                            source.resolved,
                        )
                        .ok()
                    });
                    let return_type = type_expr_to_type_with_substitutions(
                        &primitive.return_type,
                        source.resolved,
                        None,
                        &HashMap::new(),
                    );
                    if declared.is_none()
                        && !type_may_retain_fresh_result_storage(&return_type, source.resolved)
                    {
                        continue;
                    }
                    let provenance = abstract_bodyless_result_provenance(declared);
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
                                ResultProvenanceInputs::parameters(&method.parameters.parameters),
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
                                let return_type = type_expr_to_type_in_environment(
                                    &method.return_type,
                                    source.resolved,
                                    &environment,
                                );
                                Some(result_with_declared_fallback(
                                    inferred,
                                    declared,
                                    &return_type,
                                    source.resolved,
                                ))
                            }
                            None => declared,
                        };
                        let provenance = if method.body.is_none() {
                            let return_type = type_expr_to_type_in_environment(
                                &method.return_type,
                                source.resolved,
                                &environment,
                            );
                            match provenance {
                                Some(provenance) => Some(provenance),
                                None if type_may_retain_fresh_result_storage(
                                    &return_type,
                                    source.resolved,
                                ) =>
                                {
                                    Some(abstract_bodyless_result_provenance(None))
                                }
                                None => None,
                            }
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
                Item::Impl(impl_) => collect_impl_provenance_summaries(
                    impl_,
                    source.resolved,
                    previous,
                    &mut summaries,
                ),
                Item::Coerce(coerce) => {
                    let impl_ = coerce.callable_impl();
                    collect_impl_provenance_summaries(
                        &impl_,
                        source.resolved,
                        previous,
                        &mut summaries,
                    );
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
                                ResultProvenanceInputs::parameters(&function.parameters.parameters),
                                source.resolved,
                            )
                            .ok()
                        });
                        let callable = CallableId::declared_at(function_summary_key(function));
                        summaries.insert_result(
                            callable,
                            result_with_declared_fallback(
                                provenance,
                                declared,
                                &return_type,
                                source.resolved,
                            ),
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
                                ResultProvenanceInputs::literal(literal),
                                source.resolved,
                            )
                            .ok()
                        });
                        let callable = CallableId::declared_at(literal.span);
                        summaries.insert_result(
                            callable,
                            result_with_declared_fallback(
                                provenance,
                                declared,
                                &return_type,
                                source.resolved,
                            ),
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

fn collect_impl_provenance_summaries(
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    previous: &CallableProvenanceSummaries,
    summaries: &mut CallableProvenanceSummaries,
) {
    for member in &impl_.members {
        let ImplMember::Method(method) = member else {
            continue;
        };
        let Some(body) = &method.body else {
            continue;
        };
        let environment = environment_for_method(method, resolved, impl_);
        let return_type =
            type_expr_to_type_in_environment(&method.return_type, resolved, &environment);
        let provenance = borrow_return_provenance_for_callable_body(
            body,
            &return_type,
            resolved,
            &environment,
            previous,
        )
        .unwrap_or(ValueProvenance::Independent);
        let declared = method.result_provenance.as_ref().and_then(|clause| {
            result_provenance_contract(
                clause,
                Some(method),
                ResultProvenanceInputs::parameters(&method.parameters.parameters),
                resolved,
            )
            .ok()
        });
        let callable = CallableId::declared_at(method.name_span);
        summaries.insert_result(
            callable,
            result_with_declared_fallback(provenance, declared, &return_type, resolved),
        );
        collect_retained_input_mutations(
            body,
            Some(&method.receiver),
            &method.parameters.parameters,
            resolved,
            &environment,
            previous,
            summaries,
            callable,
        );
    }
}

fn result_with_declared_fallback(
    inferred: ValueProvenance,
    declared: Option<ValueProvenance>,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> ValueProvenance {
    let Some(declared) = declared else {
        return inferred;
    };
    match (inferred, return_type) {
        (
            ValueProvenance::Fallible { success, error },
            Type::Fallible {
                success: success_type,
                error: error_type,
            },
        ) => {
            let success = success.map(|value| {
                Box::new(
                    if type_may_carry_result_provenance(success_type, resolved) {
                        result_with_declared_fallback(
                            *value,
                            Some(declared.clone()),
                            success_type,
                            resolved,
                        )
                    } else {
                        *value
                    },
                )
            });
            let error = error.map(|value| {
                Box::new(if type_may_carry_result_provenance(error_type, resolved) {
                    result_with_declared_fallback(
                        *value,
                        Some(declared.clone()),
                        error_type,
                        resolved,
                    )
                } else {
                    *value
                })
            });
            ValueProvenance::Fallible { success, error }
        }
        (inferred, _) => {
            if has_exact_external_origin(&inferred) {
                return inferred;
            }
            let mut declared = declared;
            declared.merge(&inferred.retain_only_result_allocations());
            declared
        }
    }
}

fn has_exact_external_origin(provenance: &ValueProvenance) -> bool {
    match provenance {
        ValueProvenance::Independent => false,
        ValueProvenance::Origins(origins) => origins.iter().any(|origin| {
            matches!(
                origin,
                StorageOrigin::Input(_)
                    | StorageOrigin::InputWithCurrentFallback(_)
                    | StorageOrigin::Static
            )
        }),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            fallback.as_deref().is_some_and(has_exact_external_origin)
                || fields.values().any(has_exact_external_origin)
                || elements.values().any(has_exact_external_origin)
        }
        ValueProvenance::Fallible { success, error } => {
            success.as_deref().is_some_and(has_exact_external_origin)
                || error.as_deref().is_some_and(has_exact_external_origin)
        }
    }
}

fn abstract_bodyless_result_provenance(declared: Option<ValueProvenance>) -> ValueProvenance {
    declared.unwrap_or_else(|| {
        ValueProvenance::Independent
            .with_returned_allocation_from(ValueProvenance::current_allocation_context())
    })
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
