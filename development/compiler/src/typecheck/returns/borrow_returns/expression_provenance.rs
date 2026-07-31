use super::*;

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_expression(
    expression: &Expr,
    ty: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if !type_contains_borrow_like(ty, resolved) {
        return None;
    }

    match unwrap_group(expression) {
        Expr::Borrow(_) => borrow_return_provenance_for_direct_borrow(expression, resolved),
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
        Expr::StringLiteral(_) => Some(BorrowReturnProvenance::Static),
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
            (!fields.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
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
            (!elements.is_empty()).then_some(BorrowReturnProvenance::Aggregate {
                fallback: None,
                fields: BTreeMap::new(),
                elements,
            })
        }
        Expr::Call(call) if is_enum_variant_call(call, resolved) => {
            let mut provenance = None;
            for argument in &call.arguments {
                let argument_type = expression_type(argument, resolved, environment);
                merge_borrow_return_provenance(
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
            merge_borrow_return_provenance(
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
            merge_borrow_return_provenance(
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
            merge_borrow_return_provenance(
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
                merge_borrow_return_provenance(
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
                merge_borrow_return_provenance(
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
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
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
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
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
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
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
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
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
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    let signature = resolved_call_signature(resolved, call, environment)?;
    let return_type = call_return_type(call, &signature, resolved, environment);
    if let Some(declaration_span) = signature.declaration_span
        && let Some(summary) = summaries.get(&declaration_span)
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
        merge_borrow_return_provenance(&mut provenance, Some(receiver_provenance));
    }

    for (argument, parameter) in call.arguments.iter().zip(&signature.signature.parameters) {
        let argument_type = expression_type(argument, resolved, environment);
        if !type_contains_borrow_like(&argument_type, resolved)
            && !type_expr_contains_borrow_like(
                &parameter.ty,
                resolved,
                &HashMap::new(),
                &mut HashSet::new(),
            )
        {
            continue;
        }

        merge_borrow_return_provenance(
            &mut provenance,
            borrow_return_provenance_for_borrowed_input(
                argument,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            ),
        );
    }

    match return_type {
        Type::Fallible { success, error } => {
            let success_provenance = type_contains_borrow_like(&success, resolved)
                .then(|| provenance.clone())
                .flatten();
            let error_provenance = type_contains_borrow_like(&error, resolved)
                .then_some(provenance)
                .flatten();
            borrow_return_fallible_provenance(success_provenance, error_provenance)
        }
        _ => provenance,
    }
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_call_summary(
    summary: &BorrowReturnProvenance,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    match summary {
        BorrowReturnProvenance::Static => Some(BorrowReturnProvenance::Static),
        BorrowReturnProvenance::Escaping { .. } => None,
        BorrowReturnProvenance::Fallible { success, error } => {
            let mapped_success = success.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            let mapped_error = error.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            borrow_return_fallible_provenance(mapped_success, mapped_error)
        }
        BorrowReturnProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let mapped_fallback = fallback.as_deref().and_then(|provenance| {
                borrow_return_provenance_for_call_summary(
                    provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                )
            });
            let mut mapped_fields = BTreeMap::new();
            for (field, field_provenance) in fields {
                if let Some(mapped_field) = borrow_return_provenance_for_call_summary(
                    field_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_fields.insert(field.clone(), mapped_field);
                }
            }
            let mut mapped_elements = BTreeMap::new();
            for (index, element_provenance) in elements {
                if let Some(mapped_element) = borrow_return_provenance_for_call_summary(
                    element_provenance,
                    call,
                    signature,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ) {
                    mapped_elements.insert(*index, mapped_element);
                }
            }
            if mapped_fallback.is_none() && mapped_fields.is_empty() && mapped_elements.is_empty() {
                None
            } else {
                Some(BorrowReturnProvenance::Aggregate {
                    fallback: mapped_fallback.map(Box::new),
                    fields: mapped_fields,
                    elements: mapped_elements,
                })
            }
        }
        BorrowReturnProvenance::InputBorrow { sources } => {
            let mut provenance = None;
            for source in sources {
                merge_borrow_return_provenance(
                    &mut provenance,
                    borrow_return_provenance_for_call_input(
                        source,
                        call,
                        signature,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                    ),
                );
            }
            provenance
        }
    }
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_call_input(
    source: &str,
    call: &crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> Option<BorrowReturnProvenance> {
    if signature.kind == crate::typecheck::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && method.receiver.name == source
        && let Some(member) = method_member_for_call(call)
    {
        return borrow_return_provenance_for_borrowed_input(
            &member.object,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    for (index, parameter) in signature.signature.parameters.iter().enumerate() {
        if parameter.name == source {
            return call.arguments.get(index).and_then(|argument| {
                borrow_return_provenance_for_borrowed_input(
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
