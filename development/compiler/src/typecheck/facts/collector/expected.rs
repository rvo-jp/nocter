use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_expression_facts_with_expected(
        &mut self,
        expression: &Expr,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        self.collect_expression_facts_in_context(expression, environment, return_type);
        self.collect_expected_expression_facts(expression, expected, environment, return_type);
    }

    pub(in crate::typecheck::facts::collector) fn collect_expected_expression_facts(
        &mut self,
        expression: &Expr,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        if let Some(selected) = crate::typecheck::coercions::select_expression_coercion(
            expected,
            expression,
            self.resolved,
            environment,
        ) {
            self.record_coercion_plan(expression.span(), selected);
            return;
        }
        match expression {
            Expr::Closure(closure) => {
                let Type::Closure(closure_type) = expected else {
                    return;
                };
                self.record_expression_type(expression.span(), expected);
                self.record_closure_plan(closure, closure_type);
                let parameter_types = closure_type
                    .parameters
                    .iter()
                    .map(|ty| {
                        type_expr_to_type_with_substitutions(
                            ty,
                            self.resolved,
                            None,
                            &HashMap::new(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut closure_environment =
                    crate::typecheck::closures::environment_for_closure_with_parameters(
                        closure,
                        self.resolved,
                        environment,
                        &parameter_types,
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
                let result_type = type_expr_to_type_with_substitutions(
                    &closure_type.return_type,
                    self.resolved,
                    None,
                    &HashMap::new(),
                );
                self.collect_block_facts(
                    &closure.body,
                    &mut closure_environment,
                    Some(&result_type),
                );
            }
            Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_) => {
                let ty = crate::typecheck::literals::literal_expression_type_with_expected(
                    expression,
                    Some(expected),
                    self.resolved,
                    environment,
                );
                self.record_expression_type(expression.span(), &ty);
            }
            Expr::Group(expression) => {
                self.collect_expected_expression_facts(
                    &expression.expression,
                    expected,
                    environment,
                    return_type,
                );
            }
            Expr::Propagate(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Force(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Catch(expression) => {
                let expected_attempt = expected_attempt_type(
                    &expression.expression,
                    expected,
                    self.resolved,
                    environment,
                );
                self.collect_expected_expression_facts(
                    &expression.expression,
                    &expected_attempt,
                    environment,
                    return_type,
                );
            }
            Expr::Call(call) => {
                self.record_expected_generic_function_call_specialization(
                    call,
                    expected,
                    environment,
                );
                self.collect_call_argument_facts(call, Some(expected), environment, return_type);
            }
            Expr::If(expression) => {
                let mut then_environment = environment.clone();
                self.collect_expected_block_result_facts(
                    &expression.then_block,
                    expected,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        else_block,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            Expr::IfIs(expression) => {
                let mut then_environment =
                    environment_for_if_is_binding(expression, self.resolved, environment);
                self.collect_expected_block_result_facts(
                    &expression.then_block,
                    expected,
                    &mut then_environment,
                    return_type,
                );
                if let Some(else_block) = &expression.else_block {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        else_block,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            Expr::Match(expression) => {
                for arm in &expression.arms {
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &expression.expression,
                        self.resolved,
                        environment,
                    );
                    self.collect_expected_block_result_facts(
                        &arm.body,
                        expected,
                        &mut arm_environment,
                        return_type,
                    );
                }
                if let Some(wildcard_arm) = &expression.wildcard_arm {
                    let mut else_environment = environment.clone();
                    self.collect_expected_block_result_facts(
                        &wildcard_arm.body,
                        expected,
                        &mut else_environment,
                        return_type,
                    );
                }
            }
            _ => {}
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_expected_block_result_facts(
        &mut self,
        block: &Block,
        expected: &Type,
        environment: &mut TypeEnvironment,
        return_type: Option<&Type>,
    ) {
        for statement in &block.statements {
            self.collect_statement_facts(statement, environment, return_type);
        }
        if let Some(result) = &block.result {
            self.collect_expression_facts_with_expected(result, expected, environment, return_type);
        }
    }
}
