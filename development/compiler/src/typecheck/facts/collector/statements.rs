use super::*;

impl TypecheckFactCollector<'_> {
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
                self.collect_binding_statement_facts(statement, environment, return_type)
            }
            Stmt::Assignment(statement) => {
                self.collect_expression_facts_in_context(
                    &statement.target,
                    environment,
                    return_type,
                );
                self.collect_expression_facts_in_context(
                    &statement.value,
                    environment,
                    return_type,
                );
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
    ) {
        let expected_initializer_type = statement.ty.as_ref().map(|ty| {
            self.collect_type_expr_references(ty);
            type_expr_to_type_in_environment(ty, self.resolved, environment)
        });
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

        let binding_type =
            continuing_binding_type(statement, initializer_type, self.resolved, environment);
        let is_mutable = binding_kind_is_mutable(statement.kind);
        self.record_binding(statement.name_span, &binding_type, is_mutable);
        if let Some(ty) = &statement.ty {
            self.facts
                .binding_type_exprs
                .insert(statement.name_span, ty.clone());
        }
        environment.define_binding(statement.name.clone(), binding_type, is_mutable);
    }
}
