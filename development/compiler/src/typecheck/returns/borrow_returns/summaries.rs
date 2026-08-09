use super::*;
use crate::ast::LiteralDecl;

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
                    CallableId::declared_at(function_summary_key(function, source.resolved)),
                    ValueProvenance::Independent,
                ),
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        if method.body.is_some() {
                            summaries.insert_result(
                                CallableId::declared_at(
                                    source
                                        .resolved
                                        .canonical_callable_identity(method.name_span),
                                ),
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
                                CallableId::declared_at(
                                    source
                                        .resolved
                                        .canonical_callable_identity(method.name_span),
                                ),
                                ValueProvenance::Independent,
                            );
                        }
                    }
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        summaries.insert_result(
                            CallableId::declared_at(function_summary_key(
                                function,
                                source.resolved,
                            )),
                            ValueProvenance::Independent,
                        );
                    }
                    for (_, literal) in construct.literals() {
                        if literal.body.is_none() {
                            continue;
                        }
                        summaries.insert_result(
                            CallableId::declared_at(
                                source.resolved.canonical_callable_identity(literal.span),
                            ),
                            ValueProvenance::Independent,
                        );
                    }
                }
                Item::Coerce(coerce) => {
                    let impl_ = coerce.callable_impl();
                    for member in &impl_.members {
                        if let ImplMember::Method(method) = member {
                            summaries.insert_result(
                                CallableId::declared_at(
                                    source
                                        .resolved
                                        .canonical_callable_identity(method.name_span),
                                ),
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
        Item::Construct(construct) => {
            construct
                .functions()
                .filter(|(_, function)| function.body.is_some())
                .count()
                + construct.literals().count()
        }
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
                    let Some(body) = &function.body else {
                        continue;
                    };
                    let environment = environment_for_function(function, source.resolved);
                    let return_type = type_expr_to_type_in_environment(
                        &function.return_type,
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
                    let declared = function.result_provenance.as_ref().and_then(|clause| {
                        result_provenance_contract(
                            clause,
                            None,
                            ResultProvenanceInputs::parameters(&function.parameters.parameters),
                            source.resolved,
                        )
                        .ok()
                    });
                    let callable =
                        CallableId::declared_at(function_summary_key(function, source.resolved));
                    insert_body_result_summary(
                        &mut summaries,
                        callable,
                        result_with_contract_fallback(
                            provenance,
                            declared,
                            &return_type,
                            source.resolved,
                        ),
                        source.resolved,
                    );
                    collect_retained_input_mutations(
                        body,
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
                    let elided = declared
                        .is_none()
                        .then(|| {
                            elided_declaration_result_contract(
                                None,
                                ResultProvenanceInputs::parameters(
                                    &primitive.parameters.parameters,
                                ),
                                &return_type,
                                source.resolved,
                            )
                            .abstract_summary()
                        })
                        .flatten();
                    let provenance = result_provenance_summary(
                        declared.or(elided),
                        &return_type,
                        source.resolved,
                    );
                    let Some(provenance) = provenance else {
                        continue;
                    };
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
                        let return_type = type_expr_to_type_in_environment(
                            &method.return_type,
                            source.resolved,
                            &environment,
                        );
                        let provenance = if let Some(body) = &method.body {
                            borrow_return_provenance_for_callable_body(
                                body,
                                &return_type,
                                source.resolved,
                                &environment,
                                previous,
                            )
                            .map(|inferred| {
                                result_with_contract_fallback(
                                    inferred,
                                    declared,
                                    &return_type,
                                    source.resolved,
                                )
                            })
                        } else {
                            let elided = declared
                                .is_none()
                                .then(|| {
                                    elided_declaration_result_contract(
                                        Some(method),
                                        ResultProvenanceInputs::parameters(
                                            &method.parameters.parameters,
                                        ),
                                        &return_type,
                                        source.resolved,
                                    )
                                    .abstract_summary()
                                })
                                .flatten();
                            result_provenance_summary(
                                declared.or(elided),
                                &return_type,
                                source.resolved,
                            )
                        };
                        let Some(provenance) = provenance else {
                            continue;
                        };
                        let callable = CallableId::declared_at(
                            source
                                .resolved
                                .canonical_callable_identity(method.name_span),
                        );
                        insert_body_result_summary(
                            &mut summaries,
                            callable,
                            provenance,
                            source.resolved,
                        );
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
                        let Some(body) = &function.body else {
                            continue;
                        };
                        let environment = environment_for_function(function, source.resolved);
                        let return_type = type_expr_to_type_in_environment(
                            &function.return_type,
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
                        let declared = function.result_provenance.as_ref().and_then(|clause| {
                            result_provenance_contract(
                                clause,
                                None,
                                ResultProvenanceInputs::parameters(&function.parameters.parameters),
                                source.resolved,
                            )
                            .ok()
                        });
                        let callable = CallableId::declared_at(function_summary_key(
                            function,
                            source.resolved,
                        ));
                        insert_body_result_summary(
                            &mut summaries,
                            callable,
                            result_with_contract_fallback(
                                provenance,
                                declared,
                                &return_type,
                                source.resolved,
                            ),
                            source.resolved,
                        );
                        collect_retained_input_mutations(
                            body,
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
                        let Some(body) = &literal.body else {
                            continue;
                        };
                        let environment = environment_for_literal(literal, source.resolved);
                        let return_type = type_expr_to_type_in_environment(
                            &literal.return_type,
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
                        let declared = literal.result_provenance.as_ref().and_then(|clause| {
                            result_provenance_contract(
                                clause,
                                None,
                                ResultProvenanceInputs::literal(literal),
                                source.resolved,
                            )
                            .ok()
                        });
                        let contract = literal_body_result_contract(
                            declared,
                            literal,
                            &return_type,
                            source.resolved,
                        );
                        let callable = CallableId::declared_at(
                            source.resolved.canonical_callable_identity(literal.span),
                        );
                        insert_body_result_summary(
                            &mut summaries,
                            callable,
                            result_with_contract_fallback(
                                provenance,
                                contract,
                                &return_type,
                                source.resolved,
                            ),
                            source.resolved,
                        );
                        collect_retained_input_mutations(
                            body,
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
        let callable =
            CallableId::declared_at(resolved.canonical_callable_identity(method.name_span));
        insert_body_result_summary(
            summaries,
            callable,
            result_with_contract_fallback(provenance, declared, &return_type, resolved),
            resolved,
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

fn insert_body_result_summary(
    summaries: &mut CallableProvenanceSummaries,
    callable: CallableId,
    result: ValueProvenance,
    resolved: &ResolveOutput,
) {
    summaries.insert_result(
        callable,
        canonicalize_provenance_summary_inputs(result, resolved),
    );
}

/// A sequence capture is a semantic callable input, but element transfers into
/// an opaque collection are represented by the literal boundary rather than
/// ordinary parameter dataflow. Preserve an omitted unique capture contract
/// as that boundary fallback. Ordinary callable inputs and string-literal text
/// remain body-inferred so elision never invents a dependency for a fresh copy.
fn literal_body_result_contract(
    declared: Option<ValueProvenance>,
    literal: &LiteralDecl,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    declared.or_else(|| {
        literal.capture.as_ref()?;
        elided_declaration_result_contract(
            None,
            ResultProvenanceInputs::literal(literal),
            return_type,
            resolved,
        )
        .allowed_contract()
        .cloned()
    })
}

fn result_with_contract_fallback(
    inferred: ValueProvenance,
    contract: Option<ValueProvenance>,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> ValueProvenance {
    let Some(contract) = contract else {
        return inferred;
    };
    match (inferred, return_type) {
        (
            ValueProvenance::Fallible { success, error },
            Type::Fallible {
                success: success_type,
                error: _,
            },
        ) => {
            let success = success.map(|value| {
                Box::new(
                    if type_may_carry_result_provenance(success_type, resolved) {
                        result_with_contract_fallback(
                            *value,
                            Some(contract.clone()),
                            success_type,
                            resolved,
                        )
                    } else {
                        *value
                    },
                )
            });
            // `from` constrains only the successful result. Error storage is
            // tracked for escape safety, but is not part of the surface origin
            // contract.
            let error = error.map(|value| Box::new(*value));
            ValueProvenance::Fallible { success, error }
        }
        (inferred, _) => {
            if has_exact_external_origin(&inferred) {
                return inferred;
            }
            let mut contract = contract;
            contract.merge(&inferred.retain_only_result_allocations());
            contract
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

pub(in crate::typecheck) fn function_summary_key(
    function: &crate::ast::FunctionDecl,
    resolved: &ResolveOutput,
) -> ByteSpan {
    let span = if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    };
    resolved.canonical_callable_identity(span)
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
