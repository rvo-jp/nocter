use super::*;

/// Summarizes newly allocated storage retained through readwrite inputs.
///
/// Result provenance alone cannot model builders such as `Vec.push`: the call
/// returns `void`, but can attach fresh storage to its receiver. These effects
/// are declaration-relative and instantiated at each call site just like
/// result provenance.
pub(in crate::typecheck::returns) fn collect_retained_input_mutations(
    block: &Block,
    receiver: Option<&crate::ast::MethodReceiver>,
    parameters: &[crate::ast::Parameter],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    previous: &CallableProvenanceSummaries,
    summaries: &mut CallableProvenanceSummaries,
    callable: CallableId,
) {
    if collect_trusted_allocation_mutation(parameters, resolved, summaries, callable) {
        return;
    }
    let mut environment = environment.clone();
    let mut provenance = ProvenanceEnvironment::default();
    apply_borrow_return_statement_effects(
        block,
        resolved,
        &mut environment,
        &mut provenance,
        previous,
    );

    if let Some(receiver) = receiver
        && receiver.mode == MethodReceiverMode::ReadwriteBorrow
    {
        retain_input_mutation(
            receiver.name_span,
            &provenance,
            resolved,
            summaries,
            callable,
        );
    }
    for parameter in parameters {
        if type_expr_is_readwrite_input(&parameter.ty) {
            retain_input_mutation(
                parameter.name_span,
                &provenance,
                resolved,
                summaries,
                callable,
            );
        }
    }
}

fn collect_trusted_allocation_mutation(
    parameters: &[crate::ast::Parameter],
    resolved: &ResolveOutput,
    summaries: &mut CallableProvenanceSummaries,
    callable: CallableId,
) -> bool {
    let Some(crate::semantics::TrustedDeclarationRole::AllocationMutation {
        target,
        source,
        fallback_to_current,
    }) = resolved
        .trusted_declarations
        .role_definition(callable.definition())
    else {
        return false;
    };
    let Some(target) = parameters.get(target) else {
        return true;
    };
    let source = match source {
        crate::semantics::AllocationSource::CurrentContext => {
            ValueProvenance::current_allocation_context()
        }
        crate::semantics::AllocationSource::Input(index) => {
            let Some(source) = parameters.get(index) else {
                return true;
            };
            let source = InputId::resolved_at(resolved, source.name_span);
            if fallback_to_current {
                ValueProvenance::input_with_current_fallback(source)
            } else {
                ValueProvenance::input(source)
            }
        }
    };
    summaries.insert_input_mutation(
        callable,
        InputId::resolved_at(resolved, target.name_span),
        source.allocated(),
    );
    true
}

fn retain_input_mutation(
    input_span: ByteSpan,
    environment: &ProvenanceEnvironment,
    resolved: &ResolveOutput,
    summaries: &mut CallableProvenanceSummaries,
    callable: CallableId,
) {
    let Some(provenance) = environment.get(input_span).cloned() else {
        return;
    };
    let retained = provenance.retain_only_result_allocations();
    if retained.contains_result_allocation() {
        summaries.insert_input_mutation(
            callable,
            InputId::resolved_at(resolved, input_span),
            canonicalize_provenance_summary_inputs(retained, resolved),
        );
    }
}

fn type_expr_is_readwrite_input(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Borrow(borrow) if borrow.is_readwrite)
        || matches!(ty, TypeExpr::View(view) if view.is_readwrite)
}

pub(in crate::typecheck::returns) fn apply_retained_call_mutations(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let Some(call) = effect_call(expression) else {
        return;
    };
    let callable_contract;
    let signature = if let Some(signature) = resolved_call_signature(resolved, call, environment) {
        signature
    } else {
        callable_contract = match crate::typecheck::callables::callable_contract_for_call(
            call,
            resolved,
            environment,
        ) {
            Some(contract) => contract,
            None => return,
        };
        crate::typecheck::calls::CheckedCallSignature {
            signature: &callable_contract.signature,
            self_type: None,
            owner_target_ty: None,
            name: callable_contract.callee_type.display(),
            kind: crate::typecheck::calls::CheckedCallKind::Function,
            declaration_span: None,
        }
    };
    let Some(declaration) = signature.declaration_span else {
        return;
    };
    let Some(callable) = CallableId::for_declaration(resolved, declaration) else {
        return;
    };
    let Some(summary) = summaries.get(callable) else {
        return;
    };

    for (input, effect) in summary.mutated_inputs() {
        let Some(argument) = call_input_expression(input, call, &signature, resolved, environment)
        else {
            continue;
        };
        let Some(identifier) = expression_root_identifier(unwrap_input_borrow(argument)) else {
            continue;
        };
        let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
            continue;
        };
        let Some(instantiated) = instantiate_mutation_summary(
            effect,
            call,
            &signature,
            resolved,
            environment,
            provenance,
            summaries,
        ) else {
            continue;
        };
        let mut next = provenance
            .get(symbol.name_span)
            .cloned()
            .unwrap_or_else(|| {
                if matches!(symbol.kind, LocalSymbolKind::Parameter) {
                    ValueProvenance::input(InputId::resolved_at(resolved, symbol.name_span))
                } else {
                    ValueProvenance::Independent
                }
            });
        next.merge(&instantiated);
        provenance.define_binding(symbol.name_span, true, Some(next));
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_mutation_summary(
    summary: &ValueProvenance,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    instantiate_provenance_summary(summary, &mut |origin| match origin {
        StorageOrigin::Static => Some(ValueProvenance::static_storage()),
        StorageOrigin::CurrentAllocationContext => {
            Some(provenance.current_allocation_context_provenance())
        }
        StorageOrigin::Input(source) => {
            call_input_expression(*source, call, signature, resolved, environment).and_then(
                |expression| {
                    value_provenance_for_call_input(
                        unwrap_input_borrow(expression),
                        resolved,
                        environment,
                        provenance,
                        summaries,
                    )
                },
            )
        }
        StorageOrigin::InputWithCurrentFallback(_) => {
            unreachable!("conditional inputs are instantiated before origin mapping")
        }
        StorageOrigin::Allocated(_) => unreachable!("summary instantiation unwraps allocations"),
        StorageOrigin::Scope { .. } | StorageOrigin::Region { .. } | StorageOrigin::Unknown => {
            Some(ValueProvenance::unknown())
        }
    })
}

fn effect_call(expression: &Expr) -> Option<&crate::ast::CallExpr> {
    match unwrap_group(expression) {
        Expr::Call(call) => Some(call),
        Expr::Propagate(propagation) => effect_call(&propagation.expression),
        Expr::Force(force) => effect_call(&force.expression),
        Expr::Catch(catch) => effect_call(&catch.expression),
        _ => None,
    }
}

fn unwrap_input_borrow(expression: &Expr) -> &Expr {
    match unwrap_group(expression) {
        Expr::Borrow(borrow) => &borrow.expression,
        expression => expression,
    }
}

fn call_input_expression<'a>(
    source: InputId,
    call: &'a crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<&'a Expr> {
    if signature.kind == crate::typecheck::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && InputId::resolved_at(resolved, method.receiver.name_span) == source
    {
        return method_member_for_call(call).map(|member| member.object.as_ref());
    }
    signature
        .signature
        .parameters
        .iter()
        .position(|parameter| InputId::resolved_at(resolved, parameter.name_span) == source)
        .and_then(|index| call.arguments.get(index))
}
