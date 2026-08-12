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
            Expr::Closure(closure) => {
                for parameter in &closure.parameters {
                    if let Some(ty) = &parameter.ty {
                        self.collect_type_expr_references(ty);
                    }
                }
                if let Some(return_type) = &closure.return_type {
                    self.collect_type_expr_references(return_type);
                }
                let closure_type = expression_type(expression, self.resolved, environment);
                if let Type::Closure(ty) = &closure_type {
                    self.record_closure_plan(closure, ty);
                    self.record_expression_type(expression.span(), &closure_type);
                }
                let mut closure_environment = crate::typecheck::closures::environment_for_closure(
                    closure,
                    self.resolved,
                    environment,
                );
                for capture in &closure.captures {
                    self.record_environment_binding(
                        capture.name_span,
                        &capture.name,
                        &closure_environment,
                    );
                }
                for parameter in &closure.parameters {
                    self.record_environment_binding(
                        parameter.name_span,
                        &parameter.name,
                        &closure_environment,
                    );
                }
                let return_type = closure.return_type.as_ref().map(|ty| {
                    type_expr_to_type_in_environment(ty, self.resolved, &closure_environment)
                });
                self.collect_block_facts(
                    &closure.body,
                    &mut closure_environment,
                    return_type.as_ref(),
                );
            }
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
                    &expression.binding,
                    &expression.expression,
                    self.resolved,
                    environment,
                );
                if let crate::ast::CatchBinding::Named { name, span } = &expression.binding {
                    self.record_environment_binding(*span, name, &catch_environment);
                }
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
                if expression.is_readwrite
                    && let Expr::Index(index) = super::unwrap_group(&expression.expression)
                {
                    self.collect_declared_index_call_facts(
                        index,
                        crate::typecheck::indexing::IndexAccess::Readwrite,
                        environment,
                        return_type,
                    );
                    self.record_index_plan(
                        index,
                        crate::typecheck::indexing::IndexAccess::Readwrite,
                        environment,
                    );
                }
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
                if let Some(semantics) =
                    crate::typecheck::operators::comparison_semantics(expression.operator)
                {
                    let selected = crate::typecheck::operators::resolved_comparison_method(
                        expression,
                        self.resolved,
                        environment,
                    );
                    let adjustment = selected.as_ref().and_then(|selected| {
                        let parameter = selected.method.signature.parameters.first()?;
                        let expected = type_expr_to_type_with_substitutions(
                            &parameter.ty,
                            self.resolved,
                            Some(&selected.self_type),
                            &std::collections::HashMap::new(),
                        );
                        let semantic_right = if semantics.reverse_operands {
                            expression.left.as_ref()
                        } else {
                            expression.right.as_ref()
                        };
                        let actual = expression_type(semantic_right, self.resolved, environment);
                        crate::typecheck::operators::comparison_operand_adjustment(
                            &expected,
                            semantic_right,
                            &actual,
                            self.resolved,
                            environment,
                        )
                    });
                    if selected.is_some() && adjustment.is_some() {
                        let call =
                            crate::typecheck::operators::synthetic_comparison_call(expression);
                        self.collect_expression_facts_in_context(
                            &Expr::Call(call),
                            environment,
                            return_type,
                        );
                    }
                    let mut free_type_parameters = std::collections::HashSet::new();
                    let left_type = expression_type(&expression.left, self.resolved, environment);
                    let right_type = expression_type(&expression.right, self.resolved, environment);
                    if let (Some(left_ty), Some(right_ty)) = (
                        type_to_type_expr_allowing_parameters(
                            &left_type,
                            expression.left.span(),
                            &mut free_type_parameters,
                        ),
                        type_to_type_expr_allowing_parameters(
                            &right_type,
                            expression.right.span(),
                            &mut free_type_parameters,
                        ),
                    ) {
                        let method = selected.as_ref().and_then(|selected| {
                            crate::typecheck::operators::operator_method_fact(
                                selected,
                                expression.operator_span,
                                self.resolved,
                            )
                        });
                        let (semantic_left_type, semantic_right_type) =
                            if semantics.reverse_operands {
                                (&right_type, &left_type)
                            } else {
                                (&left_type, &right_type)
                            };
                        let requirement_span = match semantics.kind {
                            crate::ast::ComparisonOperatorKind::Equality => environment
                                .equality_requirement_span(semantic_left_type, semantic_right_type),
                            crate::ast::ComparisonOperatorKind::StrictOrder => environment
                                .ordering_requirement_span(semantic_left_type, semantic_right_type),
                        };
                        if (method.is_some() && adjustment.is_some()) || requirement_span.is_some()
                        {
                            let semantic_left_span = if semantics.reverse_operands {
                                expression.right.span()
                            } else {
                                expression.left.span()
                            };
                            let semantic_right_span = if semantics.reverse_operands {
                                expression.left.span()
                            } else {
                                expression.right.span()
                            };
                            let semantic_left_conversion = self
                                .facts
                                .conversion_plans
                                .get(&semantic_left_span)
                                .cloned();
                            let semantic_right_conversion = adjustment
                                .as_ref()
                                .and_then(|adjustment| adjustment.conversion.clone())
                                .and_then(|conversion| {
                                    super::super::typecheck_conversion_plan(
                                        semantic_right_span,
                                        semantic_right_span,
                                        None,
                                        conversion,
                                    )
                                })
                                .or_else(|| {
                                    self.facts
                                        .conversion_plans
                                        .get(&semantic_right_span)
                                        .cloned()
                                });
                            let (left_conversion, right_conversion) = if semantics.reverse_operands
                            {
                                (semantic_right_conversion, semantic_left_conversion)
                            } else {
                                (semantic_left_conversion, semantic_right_conversion)
                            };
                            let right_implicit_readonly_borrow =
                                selected.as_ref().is_some_and(|selected| {
                                    let expected = crate::typecheck::model::Type::Borrow {
                                        is_readwrite: false,
                                        inner: Box::new(selected.self_type.clone()),
                                    };
                                    crate::typecheck::operators::comparison_operand_adjustment(
                                        &expected,
                                        &expression.right,
                                        &right_type,
                                        self.resolved,
                                        environment,
                                    )
                                    .is_some_and(|adjustment| adjustment.implicit_readonly_borrow)
                                });
                            self.facts.comparison_plans.insert(
                                expression.operator_span,
                                TypecheckComparisonPlan {
                                    kind: semantics.kind,
                                    operator_span: expression.operator_span,
                                    call_span: expression.span,
                                    left_span: expression.left.span(),
                                    right_span: expression.right.span(),
                                    left_ty,
                                    right_ty,
                                    method,
                                    right_implicit_readonly_borrow,
                                    left_conversion,
                                    right_conversion,
                                    reverse_operands: semantics.reverse_operands,
                                    invert_result: semantics.invert_result,
                                },
                            );
                        }
                    }
                }
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
                let target =
                    type_expr_to_type_in_environment(&expression.ty, self.resolved, environment);
                if let Ok(selected) = crate::typecheck::conversions::select_expression_conversion(
                    crate::typecheck::conversions::ConversionMode::Explicit,
                    &target,
                    &expression.expression,
                    self.resolved,
                    environment,
                ) {
                    self.record_conversion_plan(
                        expression.span,
                        expression.expression.span(),
                        Some(expression.as_span),
                        selected,
                    );
                }
            }
            Expr::Call(expression) => {
                if let Some(method) = method_member_for_call(expression)
                    && let Some(selected_method) =
                        resolved_method_call(self.resolved, expression, environment)
                {
                    let owner = selected_method.owner;
                    let resolved_method = selected_method.method;
                    self.facts
                        .method_call_targets
                        .insert(method.member_span, resolved_method.name_span);
                    if let Some(coercion) = selected_method.receiver_coercion {
                        let receiver_type =
                            expression_type(&method.object, self.resolved, environment);
                        self.record_conversion_plan(
                            method.object.span(),
                            method.object.span(),
                            None,
                            crate::typecheck::conversions::selected_receiver_coercion(
                                &receiver_type,
                                coercion,
                            ),
                        );
                    }
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
                    if owner.kind == crate::resolve::TypeSymbolKind::Interface
                        || !resolved_method.signature.generic_parameters.is_empty()
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
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((_, resolved_function)) =
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
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else if let Some(method) = method_member_for_call(expression)
                    && let Some((_, variant)) =
                        resolved_enum_variant_for_member(method, self.resolved)
                {
                    self.record_enum_variant_reference(method.member_span, variant);
                    self.collect_expression_facts_in_context(
                        &method.object,
                        environment,
                        return_type,
                    );
                } else {
                    if let Some(contract) = crate::typecheck::callables::callable_contract_for_call(
                        expression,
                        self.resolved,
                        environment,
                    ) && let Some(fact) = callable_call_fact(expression, &contract)
                    {
                        self.facts.callable_calls.insert(expression.span, fact);
                    } else if let Some(symbol) = self.resolved.symbol_for_call(expression) {
                        match &symbol.kind {
                            SymbolKind::Function(signature) => {
                                self.record_function_call_reference(
                                    expression,
                                    symbol.declaration_span,
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
                if let Some((_, variant)) =
                    resolved_enum_variant_for_member(expression, self.resolved)
                {
                    self.record_enum_variant_reference(expression.member_span, variant);
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
                self.collect_declared_index_call_facts(
                    expression,
                    crate::typecheck::indexing::IndexAccess::Readonly,
                    environment,
                    return_type,
                );
                self.record_index_plan(
                    expression,
                    crate::typecheck::indexing::IndexAccess::Readonly,
                    environment,
                );
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression_facts_in_context(element, environment, return_type);
                }
            }
            Expr::TypedSequenceLiteral(expression) => {
                self.collect_type_expr_references(&expression.target);
                let expected_element =
                    crate::typecheck::literals::typed_sequence_literal_element_type(
                        expression,
                        None,
                        self.resolved,
                        environment,
                    );
                for element in &expression.elements {
                    if let Some(expected) = &expected_element
                        && crate::typecheck::literals::sequence_spread(element).is_none()
                    {
                        self.collect_expression_facts_with_expected(
                            element,
                            expected,
                            environment,
                            return_type,
                        );
                    } else {
                        self.collect_expression_facts_in_context(element, environment, return_type);
                    }
                    if let Some(spread) = crate::typecheck::literals::sequence_spread(element)
                        && let Ok(resolution) = crate::typecheck::iteration::resolve_sequence_spread(
                            spread,
                            self.resolved,
                            environment,
                        )
                        && let Some(plan) = sequence_spread_fact(spread, &resolution, self.resolved)
                    {
                        self.facts.sequence_spread_plans.insert(spread.span, plan);
                        self.record_drop_type_specialization(
                            spread.span,
                            &resolution.iteration.iterator_type,
                        );
                    }
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
                    if let Some(expected) =
                        struct_literal_field_type(expression, field, self.resolved, environment)
                    {
                        self.collect_expression_facts_with_expected(
                            &field.value,
                            &expected,
                            environment,
                            return_type,
                        );
                    } else {
                        self.collect_expression_facts_in_context(
                            &field.value,
                            environment,
                            return_type,
                        );
                    }
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
        if let Some((owner, variant)) = resolved_enum_variant_for_call(call, self.resolved) {
            if call.arguments.len() != variant.payload.len() {
                for argument in &call.arguments {
                    self.collect_expression_facts_in_context(argument, environment, return_type);
                }
                return;
            }
            let substitutions = enum_variant_call_substitutions(
                owner,
                variant,
                &call.arguments,
                expected_return_type,
                self.resolved,
                environment,
            );
            for (argument, parameter) in call.arguments.iter().zip(&variant.payload) {
                let expected = type_expr_to_type_with_substitutions(
                    &parameter.ty,
                    self.resolved,
                    environment.self_type(),
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
            return;
        }
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

    pub(in crate::typecheck::facts::collector) fn record_closure_plan(
        &mut self,
        expression: &crate::ast::ClosureExpr,
        ty: &crate::ast::ClosureTypeExpr,
    ) {
        self.facts.closure_plans.insert(
            expression.span,
            TypecheckClosurePlan {
                expression_span: expression.span,
                ty: ty.clone(),
                target_name: format!("{}.call", ty.identity_name()),
            },
        );
    }

    pub(in crate::typecheck::facts::collector) fn collect_declared_index_call_facts(
        &mut self,
        expression: &crate::ast::IndexExpr,
        access: crate::typecheck::indexing::IndexAccess,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        let Ok(selected) = crate::typecheck::indexing::select_index_expression(
            expression,
            access,
            self.resolved,
            environment,
        ) else {
            return;
        };
        if selected.projection != crate::typecheck::indexing::IndexProjection::Declared {
            return;
        }
        let call = crate::typecheck::indexing::synthetic_index_call(expression, access);
        self.collect_expression_facts_in_context(&Expr::Call(call), environment, return_type);
    }
}

fn sequence_spread_fact(
    spread: &crate::ast::UnaryExpr,
    resolution: &crate::typecheck::iteration::SequenceSpreadResolution,
    resolved: &crate::resolve::ResolveOutput,
) -> Option<TypecheckSequenceSpreadPlan> {
    let mut free_type_parameters = std::collections::HashSet::new();
    let source_type = type_to_type_expr_allowing_parameters(
        &resolution.iteration.source_type,
        spread.operand.span(),
        &mut free_type_parameters,
    )?;
    let iterator_type = type_to_type_expr_allowing_parameters(
        &resolution.iteration.iterator_type,
        spread.span,
        &mut free_type_parameters,
    )?;
    let iterator_item_type = type_to_type_expr_allowing_parameters(
        &resolution.iteration.item_type,
        spread.span,
        &mut free_type_parameters,
    )?;
    let pack_item_type = type_to_type_expr_allowing_parameters(
        &resolution.pack_item_type,
        spread.span,
        &mut free_type_parameters,
    )?;
    Some(TypecheckSequenceSpreadPlan {
        spread_span: spread.span,
        operator_span: spread.operator_span,
        source_span: spread.operand.span(),
        mode: match resolution.mode {
            crate::typecheck::iteration::SequenceSpreadMode::Copy => {
                TypecheckSequenceSpreadMode::Copy
            }
            crate::typecheck::iteration::SequenceSpreadMode::Readonly => {
                TypecheckSequenceSpreadMode::Readonly
            }
            crate::typecheck::iteration::SequenceSpreadMode::Move => {
                TypecheckSequenceSpreadMode::Move
            }
        },
        source_mode: match resolution.iteration.source_mode {
            crate::typecheck::iteration::CollectionIterationSourceMode::Direct => {
                TypecheckCollectionForSourceMode::Direct
            }
            crate::typecheck::iteration::CollectionIterationSourceMode::ReadonlyConversion => {
                TypecheckCollectionForSourceMode::ReadonlyConversion
            }
            crate::typecheck::iteration::CollectionIterationSourceMode::ReadwriteConversion => {
                TypecheckCollectionForSourceMode::ReadwriteConversion
            }
            crate::typecheck::iteration::CollectionIterationSourceMode::OwnedConversion => {
                TypecheckCollectionForSourceMode::OwnedConversion
            }
        },
        source_type: source_type.clone(),
        iterator_type: iterator_type.clone(),
        iterator_item_type,
        pack_item_type,
        conversion: resolution.iteration.conversion.as_ref().map(|method| {
            super::statements::iteration_method_fact(
                method,
                source_type.clone(),
                &free_type_parameters,
                resolved,
            )
        }),
        exact_size: super::statements::iteration_method_fact(
            &resolution.exact_size,
            iterator_type.clone(),
            &free_type_parameters,
            resolved,
        ),
        step: super::statements::iteration_method_fact(
            &resolution.iteration.step,
            iterator_type,
            &free_type_parameters,
            resolved,
        ),
    })
}
