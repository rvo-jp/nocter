use super::*;

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_expression(
    expression: &Expr,
    _ty: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    borrow_return_provenance_for_expression_unfiltered(
        expression,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
}

fn borrow_return_provenance_for_expression_unfiltered(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    match unwrap_group(expression) {
        Expr::Borrow(_) => borrow_return_provenance_for_direct_borrow(
            expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Identifier(identifier) => borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        ),
        Expr::Force(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Propagate(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Catch(expression) => borrow_return_success_provenance_for_expression(
            &expression.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::TypedSequenceLiteral(literal) => borrow_return_provenance_for_typed_literal(
            literal.span,
            literal.using.as_ref(),
            false,
            Some(&literal.elements),
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::TypedStringLiteral(literal) => borrow_return_provenance_for_typed_literal(
            literal.span,
            literal.using.as_ref(),
            true,
            None,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::StringLiteral(_) => Some(ValueProvenance::static_storage()),
        Expr::InterpolatedString(_) => Some(
            borrow_provenance
                .current_allocation_context_provenance()
                .allocated(),
        ),
        Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => Some(ValueProvenance::Independent),
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            let operand_type = expression_type(&unary.operand, resolved, environment);
            borrow_return_provenance_for_expression(
                &unary.operand,
                &operand_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            )
        }
        Expr::Unary(_) => Some(ValueProvenance::Independent),
        Expr::Binary(binary) => {
            let mut provenance = borrow_return_provenance_for_expression(
                &binary.left,
                &expression_type(&binary.left, resolved, environment),
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_provenance(
                &mut provenance,
                borrow_return_provenance_for_expression(
                    &binary.right,
                    &expression_type(&binary.right, resolved, environment),
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::TypeConversion(conversion) => borrow_return_provenance_for_expression(
            &conversion.expression,
            &expression_type(&conversion.expression, resolved, environment),
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::StructLiteral(literal) => {
            let mut fields = BTreeMap::new();
            for field in &literal.fields {
                let field_type = expression_type(&field.value, resolved, environment);
                if let Some(field_provenance) = borrow_return_provenance_for_expression(
                    &field.value,
                    &field_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    fields.insert(field.name.clone(), field_provenance);
                }
            }
            (!fields.is_empty()).then_some(ValueProvenance::Aggregate {
                fallback: None,
                fields,
                elements: BTreeMap::new(),
            })
        }
        Expr::Member(member) => borrow_return_provenance_for_member(
            member,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Index(index) => borrow_return_provenance_for_index(
            index,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::ArrayLiteral(literal) => {
            let mut elements = BTreeMap::new();
            for (index, element) in literal.elements.iter().enumerate() {
                let element_type = expression_type(element, resolved, environment);
                if let Some(element_provenance) = borrow_return_provenance_for_expression(
                    element,
                    &element_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    elements.insert(index, element_provenance);
                }
            }
            (!elements.is_empty()).then_some(ValueProvenance::Aggregate {
                fallback: None,
                fields: BTreeMap::new(),
                elements,
            })
        }
        Expr::Call(call) if is_enum_variant_call(call, resolved) => {
            let mut provenance = None;
            for argument in &call.arguments {
                let argument_type = expression_type(argument, resolved, environment);
                merge_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_expression(
                        argument,
                        &argument_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        Expr::Call(call) => borrow_return_provenance_for_call(
            call,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        Expr::Otherwise(expression) => {
            let mut provenance = borrow_return_provenance_for_expression(
                &expression.value,
                &expression_type(&expression.value, resolved, environment),
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    &expression.fallback,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::If(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            merge_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::IfIs(expression) => {
            let Some(else_block) = &expression.else_block else {
                return None;
            };
            let then_environment = environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut provenance = borrow_return_provenance_for_block_result(
                &expression.then_block,
                resolved,
                &then_environment,
                &then_borrow_provenance,
                summaries,
            );
            merge_provenance(
                &mut provenance,
                borrow_return_provenance_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ),
            );
            provenance
        }
        Expr::Match(expression) => {
            let mut provenance = None;
            for arm in &expression.arms {
                let arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &expression.expression,
                    resolved,
                    environment,
                    &arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                merge_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &arm.body,
                        resolved,
                        &arm_environment,
                        &arm_borrow_provenance,
                        summaries,
                    ),
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                merge_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_block_result(
                        &wildcard_arm.body,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
        _ => None,
    }
}

pub(in crate::typecheck::returns) fn borrow_return_success_provenance_for_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let expression_type = expression_type(expression, resolved, environment);
    borrow_return_provenance_for_expression(
        expression,
        &expression_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.success_provenance())
}

pub(in crate::typecheck::returns) fn borrow_return_fallible_error_provenance_for_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let expression_type = expression_type(expression, resolved, environment);
    if !matches!(expression_type, Type::Fallible { .. }) {
        return None;
    }
    borrow_return_provenance_for_expression(
        expression,
        &expression_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.fallible_error_provenance())
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_member(
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let object_type = expression_type(&member.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &member.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.field_provenance(&member.member))
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_index(
    index: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let object_type = expression_type(&index.object, resolved, environment);
    borrow_return_provenance_for_expression(
        &index.object,
        &object_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.element_provenance(index_literal_value(&index.index)))
}

pub(in crate::typecheck::returns) fn index_literal_value(expression: &Expr) -> Option<usize> {
    integer_literal_expr_value(expression).and_then(|value| usize::try_from(value).ok())
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let callable_contract;
    let signature = if let Some(signature) = resolved_call_signature(resolved, call, environment) {
        signature
    } else {
        callable_contract =
            crate::typecheck::callables::callable_contract_for_call(call, resolved, environment)?;
        crate::typecheck::calls::CheckedCallSignature {
            signature: &callable_contract.signature,
            self_type: None,
            impl_target_ty: None,
            name: callable_contract.callee_type.display(),
            kind: crate::typecheck::calls::CheckedCallKind::Function,
            declaration_span: None,
        }
    };
    let return_type = call_return_type(call, &signature, resolved, environment);
    // Compiler-owned trusted roles are authoritative even when the source
    // declaration also has a `from` clause. A low-level body can reconstruct
    // allocator state through integers, which is intentionally less precise
    // than the validated trusted allocation-source metadata.
    if let Some(provenance) = trusted_call_result_provenance(
        call,
        &signature,
        &return_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) {
        return Some(provenance);
    }
    if signature.declaration_span.is_none() {
        let explicit_contract = signature
            .signature
            .result_provenance
            .as_ref()
            .and_then(|clause| {
                crate::typecheck::provenance::result_provenance_contract_for_signature(
                    clause,
                    &signature.signature.parameters,
                )
            });
        let elided_contract = explicit_contract
            .is_none()
            .then(|| {
                crate::typecheck::provenance::elided_signature_result_contract(
                    &signature.signature.parameters,
                    &return_type,
                    resolved,
                )
                .abstract_summary()
            })
            .flatten();
        let abstract_summary = crate::typecheck::provenance::result_provenance_summary(
            explicit_contract.or(elided_contract),
            &return_type,
            resolved,
        );
        if let Some(summary) = abstract_summary {
            return borrow_return_provenance_for_call_summary(
                &summary,
                call,
                &signature,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
        }
    }
    if signature.signature.result_provenance.is_some()
        && let Some(declaration_span) = signature.declaration_span
        && let Some(summary) = summaries.result(CallableId::declared_at(declaration_span))
    {
        return borrow_return_provenance_for_call_summary(
            summary,
            call,
            &signature,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }
    if let Some(declaration_span) = signature.declaration_span
        && let Some(summary) = summaries.result(CallableId::declared_at(declaration_span))
    {
        return borrow_return_provenance_for_call_summary(
            summary,
            call,
            &signature,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    if !type_may_carry_result_provenance(&return_type, resolved) {
        return Some(ValueProvenance::Independent);
    }

    let mut provenance = None;
    if let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method_receiver_is_borrow(method)
        && let Some(member) = method_member_for_call(call)
        && let Some(receiver_provenance) = borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        )
    {
        merge_provenance(&mut provenance, Some(receiver_provenance));
    }

    for (argument, parameter) in call.arguments.iter().zip(&signature.signature.parameters) {
        let argument_type = expression_type(argument, resolved, environment);
        let parameter_is_borrow_like = type_expr_contains_borrow_like(
            &parameter.ty,
            resolved,
            &HashMap::new(),
            &mut HashSet::new(),
        );
        let argument_provenance =
            if type_contains_borrow_like(&argument_type, resolved) || parameter_is_borrow_like {
                borrow_return_provenance_for_borrowed_input(
                    argument,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            } else {
                value_provenance_for_call_input(
                    argument,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            };

        merge_provenance(&mut provenance, argument_provenance);
    }

    let provenance = match &return_type {
        Type::Fallible { success, error } => {
            let success_provenance = type_contains_borrow_like(success, resolved)
                .then(|| provenance.clone())
                .flatten();
            let error_provenance = type_contains_borrow_like(error, resolved)
                .then_some(provenance)
                .flatten();
            fallible_provenance(success_provenance, error_provenance)
        }
        _ => provenance,
    };
    apply_abstract_result_storage(
        provenance,
        &signature,
        &return_type,
        resolved,
        borrow_provenance,
    )
}

fn apply_abstract_result_storage(
    provenance: Option<ValueProvenance>,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    return_type: &Type,
    resolved: &ResolveOutput,
    borrow_provenance: &ProvenanceEnvironment,
) -> Option<ValueProvenance> {
    if signature.signature.result_provenance.is_none()
        && type_may_retain_fresh_result_storage(return_type, resolved)
    {
        let provenance = provenance.unwrap_or(match return_type {
            Type::Fallible { .. } => ValueProvenance::Fallible {
                success: None,
                error: None,
            },
            _ => ValueProvenance::Independent,
        });
        if provenance.contains_result_allocation() {
            Some(provenance)
        } else {
            Some(provenance.with_returned_allocation_from(
                borrow_provenance.current_allocation_context_provenance(),
            ))
        }
    } else {
        provenance
    }
}

fn trusted_call_result_provenance(
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let declaration = signature.declaration_span?;
    let role = resolved.trusted_declarations.role(declaration)?;
    let provenance = match role {
        crate::semantics::TrustedDeclarationRole::CurrentAllocationContext => {
            borrow_provenance.current_allocation_context_provenance()
        }
        crate::semantics::TrustedDeclarationRole::AllocationOperation { source, .. } => {
            match source {
                crate::semantics::AllocationSource::CurrentContext => {
                    borrow_provenance.current_allocation_context_provenance()
                }
                crate::semantics::AllocationSource::Input(index) => signature
                    .signature
                    .parameters
                    .get(index)
                    .and_then(|parameter| {
                        allocation_source_provenance_for_call_input(
                            InputId::declared_at(parameter.name_span),
                            call,
                            signature,
                            resolved,
                            environment,
                            borrow_provenance,
                            summaries,
                        )
                    })
                    .unwrap_or_else(ValueProvenance::unknown),
            }
            .allocated()
        }
        crate::semantics::TrustedDeclarationRole::OwnedValueTransfer { source } => {
            let parameter = signature.signature.parameters.get(source)?;
            borrow_return_provenance_for_call_input(
                InputId::declared_at(parameter.name_span),
                call,
                signature,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            )?
            .without_input_container_scopes()
        }
        crate::semantics::TrustedDeclarationRole::BorrowedProjection { source } => {
            let parameter = signature.signature.parameters.get(source)?;
            borrow_return_provenance_for_call_input(
                InputId::declared_at(parameter.name_span),
                call,
                signature,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            )?
        }
        crate::semantics::TrustedDeclarationRole::StaticResult => ValueProvenance::static_storage(),
        crate::semantics::TrustedDeclarationRole::AllocatorCapability(_)
        | crate::semantics::TrustedDeclarationRole::AllocationMutation { .. }
        | crate::semantics::TrustedDeclarationRole::RegionEnter
        | crate::semantics::TrustedDeclarationRole::RegionRelease
        | crate::semantics::TrustedDeclarationRole::AllocationAbort => return None,
        crate::semantics::TrustedDeclarationRole::IndependentFallibleError => {
            return matches!(return_type, Type::Fallible { .. }).then(|| {
                ValueProvenance::Fallible {
                    success: None,
                    error: Some(Box::new(ValueProvenance::Independent)),
                }
            });
        }
    };

    Some(match return_type {
        Type::Fallible { .. } => ValueProvenance::Fallible {
            success: Some(Box::new(provenance)),
            error: Some(Box::new(ValueProvenance::Independent)),
        },
        _ => provenance,
    })
}

pub(in crate::typecheck::returns) fn allocation_source_provenance_for_call_input(
    source: InputId,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let index = signature
        .signature
        .parameters
        .iter()
        .position(|parameter| InputId::declared_at(parameter.name_span) == source)?;
    let argument = call.arguments.get(index)?;
    let argument = match unwrap_group(argument) {
        Expr::Borrow(borrow) => &borrow.expression,
        argument => argument,
    };
    value_provenance_for_call_input(
        argument,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_call_summary(
    summary: &ValueProvenance,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    instantiate_provenance_summary(summary, &mut |origin| match origin {
        StorageOrigin::Static => Some(ValueProvenance::static_storage()),
        StorageOrigin::CurrentAllocationContext => {
            Some(borrow_provenance.current_allocation_context_provenance())
        }
        StorageOrigin::Input(source) => borrow_return_provenance_for_call_input(
            *source,
            call,
            signature,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        StorageOrigin::InputWithCurrentFallback(_) => {
            unreachable!("conditional inputs are instantiated before origin mapping")
        }
        StorageOrigin::Allocated(_) => unreachable!("summary instantiation unwraps allocations"),
        StorageOrigin::Scope { .. } | StorageOrigin::Region { .. } | StorageOrigin::Unknown => {
            Some(ValueProvenance::unknown())
        }
    })
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_call_input(
    source: InputId,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    if signature.kind == crate::typecheck::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && InputId::declared_at(method.receiver.name_span) == source
        && let Some(member) = method_member_for_call(call)
    {
        let return_type = call_return_type(call, signature, resolved, environment);
        if method_receiver_is_borrow(method) && type_contains_borrow_like(&return_type, resolved) {
            return borrow_return_provenance_for_borrowed_input(
                &member.object,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        return value_provenance_for_call_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    for (index, parameter) in signature.signature.parameters.iter().enumerate() {
        if InputId::declared_at(parameter.name_span) == source {
            return call.arguments.get(index).and_then(|argument| {
                if type_expr_contains_borrow_like(
                    &parameter.ty,
                    resolved,
                    &HashMap::new(),
                    &mut HashSet::new(),
                ) {
                    return borrow_return_provenance_for_borrowed_input(
                        argument,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    );
                }
                value_provenance_for_call_input(
                    argument,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
        }
    }

    None
}
