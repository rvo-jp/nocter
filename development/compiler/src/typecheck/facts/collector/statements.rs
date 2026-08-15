use super::*;

impl TypedHirBuilder<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_statement_facts(
        &mut self,
        statement: &Stmt,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    if let Some(return_type) = return_type {
                        self.collect_expression_facts_with_expected(
                            expression,
                            return_type,
                            environment,
                            Some(return_type),
                        );
                    } else {
                        self.collect_expression_facts(expression, environment);
                    }
                }
            }
            Stmt::Binding(statement) => {
                self.collect_binding_statement_facts(statement, environment, return_type, None)
            }
            Stmt::Assignment(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.target,
                    environment,
                    return_type,
                );
                if let Expr::Index(index) = super::unwrap_group(&statement.target) {
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
                if statement.operator == crate::ast::AssignmentOperator::Assign {
                    let expected = expression_type(&statement.target, self.resolved, environment);
                    self.collect_expression_facts_with_expected(
                        &statement.value,
                        &expected,
                        environment,
                        return_type,
                    );
                } else {
                    self.collect_expression_facts_in_context(
                        &statement.value,
                        environment,
                        return_type,
                    );
                }
            }
            Stmt::If(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.condition,
                    environment,
                    return_type,
                );

                let mut then_environment = environment.clone();
                self.collect_block_facts(&statement.then_block, &mut then_environment, return_type);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Stmt::IfIs(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
                self.record_if_is_pattern_references(statement);

                let mut then_environment =
                    environment_for_if_is_binding(statement, self.resolved, environment);
                if let Some(payload) = statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.record_payload_binding(payload, &then_environment);
                }
                self.collect_block_facts(&statement.then_block, &mut then_environment, return_type);
                if let Some(else_block) = &statement.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(else_block, &mut else_environment, return_type);
                }
            }
            Stmt::Switch(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
                for arm in &statement.arms {
                    self.record_switch_arm_pattern_references(arm);
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &statement.expression,
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
                if let Some(arm) = &statement.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_block_facts(&arm.body, &mut else_environment, return_type);
                }
            }
            Stmt::ForRange(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.start,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(&statement.end, environment, return_type);

                let mut body_environment =
                    environment_for_for_range_binding(statement, self.resolved, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::CollectionFor(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.source,
                    environment,
                    return_type,
                );
                let Ok(resolution) = crate::typecheck::iteration::resolve_collection_iteration(
                    statement,
                    self.resolved,
                    environment,
                ) else {
                    let mut body_environment = environment.clone();
                    body_environment.define(statement.name.clone(), Type::Unknown);
                    self.collect_block_facts(&statement.body, &mut body_environment, return_type);
                    return;
                };
                if let Some(plan) = collection_for_fact(statement, &resolution, self.resolved) {
                    self.intern_compiler_type_tree(&plan.source_type);
                    self.intern_compiler_type_tree(&plan.iterator_type);
                    self.intern_compiler_type_tree(&plan.item_type);
                    self.intern_compiler_type_tree(&crate::ast::TypeExpr::Optional(
                        crate::ast::OptionalType {
                            span: statement.span,
                            inner: Box::new(plan.item_type.clone()),
                        },
                    ));
                    for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
                        self.intern_compiler_type_tree(&method.self_ty);
                        if method.receiver_mode != crate::ast::MethodReceiverMode::Owned {
                            self.intern_compiler_type_tree(&crate::ast::TypeExpr::Borrow(
                                crate::ast::BorrowType {
                                    span: statement.span,
                                    is_readwrite: method.receiver_mode
                                        == crate::ast::MethodReceiverMode::ReadwriteBorrow,
                                    inner: Box::new(method.self_ty.clone()),
                                },
                            ));
                        }
                    }
                    self.facts.collection_for_plans.insert(statement.span, plan);
                }
                self.record_drop_type_specialization(
                    statement.source.span(),
                    &resolution.iterator_type,
                );
                let mut body_environment = environment_for_collection_for_binding(
                    statement,
                    resolution.item_type,
                    environment,
                );
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::LiteralPackFor(statement) => {
                let mut body_environment =
                    environment_for_literal_pack_binding(statement, environment);
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::While(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.condition,
                    environment,
                    return_type,
                );

                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::Loop(statement) => {
                let mut body_environment = environment.clone();
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::Region(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.allocator,
                    environment,
                    return_type,
                );
                let mut body_environment = environment.clone();
                body_environment.define(
                    statement.name.clone(),
                    crate::typecheck::regions::region_binding_type(
                        statement,
                        self.resolved,
                        environment,
                    ),
                );
                self.record_environment_binding(
                    statement.name_span,
                    &statement.name,
                    &body_environment,
                );
                self.collect_block_facts(&statement.body, &mut body_environment, return_type);
            }
            Stmt::Expression(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.expression,
                    environment,
                    return_type,
                );
            }
            Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_binding_statement_facts(
        &mut self,
        statement: &BindingStmt,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
        inferred_expected: Option<&Type>,
    ) {
        let expected_initializer_type = statement
            .ty
            .as_ref()
            .map(|ty| {
                self.collect_type_expr_references(ty);
                type_expr_to_type_in_environment(ty, self.resolved, environment)
            })
            .or_else(|| inferred_expected.cloned());
        if let Some(expected) = &expected_initializer_type {
            self.collect_expression_facts_with_expected(
                &statement.initializer,
                expected,
                environment,
                return_type,
            );
        } else {
            self.collect_expression_facts_in_context(
                &statement.initializer,
                environment,
                return_type,
            );
        }
        let initializer_type = expression_type(&statement.initializer, self.resolved, environment);

        let binding_type = if statement.ty.is_none()
            && initializer_type.is_unknown_or_unresolved()
            && let Some(expected) = inferred_expected
        {
            expected.clone()
        } else {
            continuing_binding_type(statement, initializer_type, self.resolved, environment)
        };
        let is_mutable = binding_kind_is_mutable(statement.kind);
        self.record_binding(statement.name_span, &binding_type, is_mutable);
        if let Some(ty) = &statement.ty {
            let symbol = self
                .resolved
                .local_symbol_id_at_name_span(statement.name_span)
                .expect("resolver omitted local declaration");
            self.facts.binding_type_exprs.insert(symbol, ty.clone());
        }
        environment.define_binding(statement.name.clone(), binding_type, is_mutable);
    }
}

pub(super) fn collection_for_fact(
    statement: &crate::ast::CollectionForStmt,
    resolution: &crate::typecheck::iteration::CollectionIterationResolution,
    resolved: &crate::resolve::ResolveOutput,
) -> Option<TypecheckCollectionForPlan> {
    let mut free_type_parameters = HashSet::new();
    let source_type = type_to_type_expr_allowing_parameters(
        &resolution.source_type,
        statement.source.span(),
        &mut free_type_parameters,
    )?;
    let iterator_type = type_to_type_expr_allowing_parameters(
        &resolution.iterator_type,
        statement.source.span(),
        &mut free_type_parameters,
    )?;
    let item_type = type_to_type_expr_allowing_parameters(
        &resolution.item_type,
        statement.name_span,
        &mut free_type_parameters,
    )?;
    let conversion = resolution.conversion.as_ref().map(|method| {
        iteration_method_fact(method, source_type.clone(), &free_type_parameters, resolved)
    });
    let step = iteration_method_fact(
        &resolution.step,
        iterator_type.clone(),
        &free_type_parameters,
        resolved,
    );
    Some(TypecheckCollectionForPlan {
        binding_span: statement.name_span,
        source_span: statement.source.span(),
        source_mode: match resolution.source_mode {
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
        source_type,
        iterator_type,
        item_type,
        conversion,
        step,
    })
}

pub(super) fn iteration_method_fact(
    resolution: &crate::typecheck::iteration::IterationMethodResolution,
    self_ty: TypeExpr,
    free_type_parameters: &HashSet<String>,
    resolved: &crate::resolve::ResolveOutput,
) -> TypecheckProtocolMethod {
    TypecheckProtocolMethod {
        def_id: resolved
            .semantic_db
            .definition_at(resolution.declaration)
            .expect("resolved iteration method must have a semantic definition"),
        declaration_span: resolution.declaration,
        target_name: resolution.target_name.clone(),
        self_ty,
        receiver_mode: resolution.receiver_mode,
        method_name: resolution.method_name.clone(),
        free_type_parameters: free_type_parameters.clone(),
    }
}
