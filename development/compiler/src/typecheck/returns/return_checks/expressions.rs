use super::*;

pub(in crate::typecheck::returns) fn check_expression_for_nested_returns(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match expression {
        Expr::Propagate(expression) => {
            check_propagation(
                sources,
                expression.operator_span,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_propagated_fallible_error_borrow_return_provenance(
                sources,
                expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Catch(expression) => {
            check_catch_operand(
                sources,
                expression.catch_span,
                &expression.expression,
                resolved,
                environment,
                diagnostics,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            let mut catch_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.catch_block,
                context,
                resolved,
                diagnostics,
                &mut catch_environment,
                &mut catch_borrow_provenance,
                summaries,
            );
            if !block_guarantees_control_exit_or_never(
                &expression.catch_block,
                resolved,
                &catch_environment,
            ) {
                diagnostics.push(catch_block_fallthrough_diagnostic(
                    sources,
                    &expression.catch_block,
                ));
            }
        }
        Expr::Force(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Borrow(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Binary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.left,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.right,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Unary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.operand,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Call(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.callee,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            for argument in &expression.arguments {
                check_expression_for_nested_returns(
                    sources,
                    argument,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::Index(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.index,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_for_nested_returns(
                    sources,
                    element,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_for_nested_returns(
                    sources,
                    &field.value,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression_for_nested_returns(
                        sources,
                        &part.expression,
                        context,
                        resolved,
                        diagnostics,
                        environment,
                        borrow_provenance,
                        summaries,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.value,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let present_borrow_provenance = borrow_provenance.clone();
            let mut fallback_environment = environment.clone();
            let mut fallback_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.fallback,
                context,
                resolved,
                diagnostics,
                &mut fallback_environment,
                &mut fallback_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![present_borrow_provenance];
            if !block_guarantees_control_exit_or_never(
                &expression.fallback,
                resolved,
                &fallback_environment,
            ) {
                incoming.push(fallback_borrow_provenance);
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Expr::If(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.condition,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &expression.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            if let Some(else_block) = &expression.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::IfIs(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment =
                environment_for_if_is_binding(expression, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                expression,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            check_block_return_statements(
                sources,
                &expression.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            if let Some(else_block) = &expression.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Match(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            for arm in &expression.arms {
                let mut arm_environment =
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
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                check_block_return_statements(
                    sources,
                    &wildcard_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}
