use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_expression_facts(
        &mut self,
        expression: &Expr,
        environment: &mut TypeEnvironment,
    ) {
        self.collect_expression_facts_in_context(expression, environment, None);
    }

    pub(in crate::typecheck::facts::collector) fn collect_expression_facts_in_context(
        &mut self,
        expression: &Expr,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        self.record_expression_type(
            expression.span(),
            &expression_type(expression, self.resolved, environment),
        );
        match expression {
            Expr::Propagate(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Force(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Catch(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                let mut catch_environment = environment_for_catch(
                    expression.error_name.clone(),
                    &expression.expression,
                    self.resolved,
                    environment,
                );
                self.record_environment_binding(
                    expression.error_span,
                    &expression.error_name,
                    &catch_environment,
                );
                self.collect_block_facts(
                    &expression.catch_block,
                    &mut catch_environment,
                    return_type,
                );
            }
            Expr::Borrow(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::Binary(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.left,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &expression.right,
                    environment,
                    return_type,
                );
            }
            Expr::Unary(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.operand,
                    environment,
                    return_type,
                );
            }
            Expr::TypeConversion(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                self.collect_type_expr_references(&expression.ty);
            }
            Expr::Call(expression) => {
                if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, resolved_method)) =
                        resolved_method_for_call(self.resolved, expression, environment)
                {
                    self.facts
                        .method_call_targets
                        .insert(method.member_span, resolved_method.name_span);
                    let kind = match resolved_method.receiver.mode {
                        MethodReceiverMode::Owned => TypecheckMethodReceiverKind::Owned,
                        MethodReceiverMode::ReadonlyBorrow => {
                            TypecheckMethodReceiverKind::ReadonlyBorrow
                        }
                        MethodReceiverMode::ReadwriteBorrow => {
                            TypecheckMethodReceiverKind::ReadwriteBorrow
                        }
                    };
                    self.facts
                        .method_call_receiver_kinds
                        .insert(method.member_span, kind);
                    let receiver_is_bounded_parameter = matches!(
                        method_self_type_for_receiver_in_environment(
                            &expression_type(&method.object, self.resolved, environment),
                            environment,
                        ),
                        Type::Parameter(_)
                    );
                    if !resolved_method.signature.generic_parameters.is_empty()
                        || receiver_is_bounded_parameter
                    {
                        self.facts
                            .generic_method_call_spans
                            .insert(method.member_span, resolved_method.name_span);
                        if let Some(specialization) = method_call_specialization(
                            expression,
                            method,
                            resolved_method,
                            self.resolved,
                            environment,
                        ) {
                            self.facts
                                .method_call_specializations
                                .insert(method.member_span, specialization);
                        }
                    }
                    self.facts.call_hover_labels.insert(
                        method.member_span,
                        method_signature_hover_label(resolved_method, owner, self.resolved),
                    );
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, resolved_function)) =
                        self.resolved.associated_function_for_call(expression)
                {
                    self.facts
                        .associated_function_targets
                        .insert(method.member_span, resolved_function.name_span);
                    self.record_generic_function_call_specialization(
                        expression,
                        resolved_function.name_span,
                        &resolved_function.target_name,
                        &resolved_function.signature,
                        None,
                        environment,
                        true,
                    );
                    self.facts.call_hover_labels.insert(
                        method.member_span,
                        associated_function_signature_hover_label(
                            owner,
                            resolved_function,
                            self.resolved,
                        ),
                    );
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((owner, variant)) =
                        resolved_enum_variant_for_member(method, self.resolved)
                {
                    self.record_enum_variant_reference(method.member_span, owner, variant);
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else {
                    if let Some(symbol) = self.resolved.symbol_for_call(expression) {
                        match &symbol.kind {
                            SymbolKind::Function(signature) => {
                                self.record_function_call_reference(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    "func",
                                    signature,
                                );
                                self.record_generic_function_call_specialization(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    signature,
                                    None,
                                    environment,
                                    true,
                                );
                            }
                            SymbolKind::Primitive(signature) => {
                                self.record_function_call_reference(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    "primitive",
                                    signature,
                                );
                                self.record_generic_function_call_specialization(
                                    expression,
                                    symbol.declaration_span,
                                    &symbol.name,
                                    signature,
                                    None,
                                    environment,
                                    false,
                                );
                            }
                            SymbolKind::Type(_) | SymbolKind::Imported(_) => {}
                        }
                    }
                    self.collect_expression_facts_in_context(
                        &expression.callee,
                        environment,
                        return_type,
                    );
                }

                self.collect_call_argument_facts(expression, None, environment, return_type);
            }
            Expr::Member(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.object,
                    environment,
                    return_type,
                );
                self.record_struct_field_member_reference(expression, environment);
                if let Some((owner, variant)) =
                    resolved_enum_variant_for_member(expression, self.resolved)
                {
                    self.record_enum_variant_reference(expression.member_span, owner, variant);
                }
            }
            Expr::Index(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.object,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &expression.index,
                    environment,
                    return_type,
                );
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression_facts_in_context(element, environment, return_type);
                }
            }
            Expr::TypedSequenceLiteral(expression) => {
                self.collect_type_expr_references(&expression.target);
                for element in &expression.elements {
                    self.collect_expression_facts_in_context(element, environment, return_type);
                }
                if let Some(using) = &expression.using {
                    self.collect_expression_facts_in_context(
                        &using.allocator,
                        environment,
                        return_type,
                    );
                }
            }
            Expr::TypedStringLiteral(expression) => {
                self.collect_type_expr_references(&expression.target);
                if let Some(using) = &expression.using {
                    self.collect_expression_facts_in_context(
                        &using.allocator,
                        environment,
                        return_type,
                    );
                }
            }
            Expr::StructLiteral(expression) => {
                self.collect_type_expr_references(&expression.ty);
                for field in &expression.fields {
                    self.record_struct_literal_field_reference(expression, field, environment);
                    self.collect_expression_facts_in_context(
                        &field.value,
                        environment,
                        return_type,
                    );
                }
            }
            Expr::Group(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
            }
            Expr::InterpolatedString(expression) => {
                self.record_interpolation_plan(expression, environment);
                for part in &expression.parts {
                    if let InterpolatedStringPart::Expression(part) = part {
                        self.collect_expression_facts_in_context(
                            &part.expression,
                            environment,
                            return_type,
                        );
                    }
                }
            }
            Expr::Otherwise(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.value,
                    environment,
                    return_type,
                );
                let mut fallback_environment = environment.clone();
                self.collect_block_facts(
                    &expression.fallback,
                    &mut fallback_environment,
                    return_type,
                );
            }
            Expr::If(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.condition,
                    environment,
                    return_type,
                );

                let mut then_environment = environment.clone();
                self.collect_block_facts(
                    &expression.then_block,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Expr::IfIs(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                self.record_if_is_pattern_references(expression);

                let mut then_environment =
                    environment_for_if_is_binding(expression, self.resolved, environment);
                if let Some(payload) = expression
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.record_payload_binding(payload, &then_environment);
                }
                self.collect_block_facts(
                    &expression.then_block,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Expr::Match(expression) => {
                self.collect_expression_facts_in_context(
                    &expression.expression,
                    environment,
                    return_type,
                );
                for arm in &expression.arms {
                    self.record_switch_arm_pattern_references(arm);
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        self.resolved,
                        environment,
                    );
                    if let Some(payload) =
                        arm.payload.as_ref().and_then(|payload| payload.binding())
                    {
                        self.record_payload_binding(payload, &arm_environment);
                    }
                    self.collect_block_facts(&arm.body, &mut arm_environment, return_type);
                }
                if let Some(arm) = &expression.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(&arm.body, &mut else_environment, return_type);
                }
            }
            Expr::Identifier(identifier) => {
                self.record_environment_binding_readonly(
                    identifier.span,
                    &identifier.name,
                    environment,
                );
            }
            Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_call_argument_facts(
        &mut self,
        call: &crate::ast::CallExpr,
        expected_return_type: Option<&Type>,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        let Some(checked) = resolved_call_signature(self.resolved, call, environment) else {
            for argument in &call.arguments {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            }
            return;
        };
        if call.arguments.len() != checked.signature.parameters.len() {
            for argument in &call.arguments {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            }
            return;
        }

        let mut substitutions =
            infer_generic_substitutions(call, &checked, self.resolved, environment);
        if let Some(expected_return_type) = expected_return_type {
            let parameters = checked
                .signature
                .generic_parameters
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            infer_type_expr_substitutions(
                &checked.signature.return_type,
                expected_return_type,
                self.resolved,
                checked.self_type.as_ref(),
                &parameters,
                &mut substitutions,
            );
        }

        for (argument, parameter) in call
            .arguments
            .iter()
            .zip(checked.signature.parameters.iter())
        {
            let expected = type_expr_to_type_with_substitutions(
                &parameter.ty,
                self.resolved,
                checked.self_type.as_ref(),
                &substitutions,
            );
            if expected.is_unknown_or_unresolved() || expected.first_unsized_part().is_some() {
                self.collect_expression_facts_in_context(argument, environment, return_type);
            } else {
                self.collect_expression_facts_with_expected(
                    argument,
                    &expected,
                    environment,
                    return_type,
                );
            }
        }
    }
}
