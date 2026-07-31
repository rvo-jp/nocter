use super::terminal::statement_guarantees_return_or_never;
use super::*;

pub(super) fn check_impl_member_return_types(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &BorrowReturnSummaries,
) {
    for member in &impl_.members {
        match member {
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, impl_);
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                let context = ReturnContext::new(
                    CallableKind::Method(impl_member_name(impl_, &method.name)),
                    type_expr_to_type_in_environment(&method.return_type, resolved, &environment),
                    method.return_type.span(),
                );
                check_fallible_success_type(sources, &context, diagnostics);
                check_block_returns(
                    sources,
                    body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    summaries,
                );
            }
            ImplMember::Drop(drop_) => {
                let context = ReturnContext::new(
                    CallableKind::Drop(impl_member_name(impl_, "drop")),
                    Type::Void,
                    drop_.binding.ty.span(),
                );
                let mut environment = environment_for_parameters_in_impl(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    impl_,
                );
                let mut borrow_provenance = BorrowReturnEnvironment::default();
                check_block_returns(
                    sources,
                    &drop_.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    summaries,
                );
            }
        }
    }
}

pub(super) fn check_fallible_success_type(
    sources: &SourceMap,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Type::Fallible { success, .. } = &context.declared_type else {
        return;
    };

    if success_type_accepts_bare_error(success) {
        diagnostics.push(fallible_success_error_diagnostic(sources, context));
    }
}

pub(super) fn success_type_accepts_bare_error(ty: &Type) -> bool {
    match ty {
        Type::Error => true,
        Type::Optional(inner) => success_type_accepts_bare_error(inner),
        _ => false,
    }
}

pub(super) fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    if context.success_type().first_unsized_part().is_some() {
        return;
    }

    let block_exits = check_block_return_statements(
        sources,
        block,
        context,
        resolved,
        diagnostics,
        environment,
        borrow_provenance,
        summaries,
    );

    if block_exits {
        return;
    }

    if let Some(result) = &block.result {
        check_body_result_return(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return;
    }

    if context.requires_explicit_return()
        && !block_guarantees_return_or_never(block, resolved, environment)
    {
        diagnostics.push(missing_return_diagnostic(sources, block.span, context));
    }
}

pub(super) fn check_block_return_statements(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) -> bool {
    for statement in &block.statements {
        check_statement_returns(
            sources,
            statement,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return true;
        }
    }
    if let Some(result) = &block.result {
        check_expression_for_nested_returns(
            sources,
            result,
            context,
            resolved,
            diagnostics,
            environment,
            borrow_provenance,
            summaries,
        );
        return expression_type(result, resolved, environment) == Type::Never;
    }

    false
}

pub(super) fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_for_nested_returns(
                    sources,
                    expression,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            check_return_statement(
                sources,
                statement,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
        }
        Stmt::Binding(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            let provenance = borrow_return_provenance_for_expression(
                &statement.initializer,
                &binding_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            borrow_provenance.define_binding(
                statement.name.clone(),
                type_contains_borrow_like(&binding_type, resolved),
                provenance,
            );
        }
        Stmt::Assignment(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.target,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.value,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(target_type) = environment.get(&identifier.name)
            {
                let provenance = borrow_return_provenance_for_expression(
                    &statement.value,
                    target_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
                borrow_provenance.define_binding(
                    identifier.name.clone(),
                    type_contains_borrow_like(target_type, resolved),
                    provenance,
                );
            }
        }
        Stmt::If(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
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
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            if !block_guarantees_return_or_never(&statement.then_block, resolved, &then_environment)
            {
                incoming.push(then_borrow_provenance);
            }
            if let Some(else_block) = &statement.else_block {
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
                if !block_guarantees_return_or_never(else_block, resolved, &else_environment) {
                    incoming.push(else_borrow_provenance);
                }
            } else {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_borrow_provenance = borrow_provenance.clone();
            define_if_is_payload_borrow_return_binding(
                statement,
                resolved,
                environment,
                &then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            if !block_guarantees_return_or_never(&statement.then_block, resolved, &then_environment)
            {
                incoming.push(then_borrow_provenance);
            }
            if let Some(else_block) = &statement.else_block {
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
                if !block_guarantees_return_or_never(else_block, resolved, &else_environment) {
                    incoming.push(else_borrow_provenance);
                }
            } else {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::Switch(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut incoming = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_borrow_provenance = borrow_provenance.clone();
                define_switch_arm_payload_borrow_return_binding(
                    arm,
                    &statement.expression,
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
                if !block_guarantees_return_or_never(&arm.body, resolved, &arm_environment) {
                    incoming.push(arm_borrow_provenance);
                }
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
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
                if !block_guarantees_return_or_never(
                    &wildcard_arm.body,
                    resolved,
                    &else_environment,
                ) {
                    incoming.push(else_borrow_provenance);
                }
            } else if !switch_statement_covers_all_variants(statement, resolved, environment) {
                incoming.push(borrow_provenance.clone());
            }
            if !incoming.is_empty() {
                borrow_provenance.join_reachable(&incoming);
            }
        }
        Stmt::While(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment = environment.clone();
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.start,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.end,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_borrow_provenance = borrow_provenance.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_borrow_provenance,
                summaries,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
        Stmt::Expression(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
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

pub(super) fn check_expression_for_nested_returns(
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

pub(super) fn check_body_result_return(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let expected = context.success_type();
    let actual = expression_type(expression, resolved, environment);

    if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
        return;
    }

    if expected == &Type::Void {
        if actual == Type::Void
            || actual == Type::Never
            || return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            )
        {
            return;
        }

        diagnostics.push(unexpected_body_result_diagnostic(
            sources, expression, context,
        ));
        return;
    }

    if expected.first_unsized_part().is_some() {
        return;
    }

    if return_expression_is_fallible_failure(expression, &actual, context, resolved, environment) {
        return;
    }

    if !is_expression_assignable(expected, expression, resolved, environment) {
        diagnostics.push(body_result_type_mismatch_diagnostic(
            sources, expression, expected, &actual, context,
        ));
        return;
    }

    check_borrow_return_provenance(
        sources,
        expression,
        &actual,
        context,
        resolved,
        environment,
        borrow_provenance,
        summaries,
        diagnostics,
    );

    if let Some(source) = implicit_non_copy_owned_value_source(expression, resolved, environment) {
        diagnostics.push(non_copy_struct_return_diagnostic(
            sources,
            expression,
            &source.source_name,
            &source.type_name,
            source.kind,
            context,
        ));
    }
}

pub(super) fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    let expected = context.success_type();
    if expected == &Type::Never {
        diagnostics.push(never_return_statement_diagnostic(
            sources, statement, context,
        ));
        return;
    }

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Unresolved(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            let actual = expression_type(expression, resolved, environment);
            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                return;
            }

            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = expression_type(expression, resolved, environment);
            if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
                return;
            }
            if expected.first_unsized_part().is_some() {
                return;
            }

            if return_expression_is_fallible_failure(
                expression,
                &actual,
                context,
                resolved,
                environment,
            ) {
                return;
            }

            if !is_expression_assignable(expected, expression, resolved, environment) {
                diagnostics.push(return_type_mismatch_diagnostic(
                    sources, expression, expected, &actual, context,
                ));
                return;
            }

            check_borrow_return_provenance(
                sources,
                expression,
                &actual,
                context,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                diagnostics,
            );

            if let Some(source) =
                implicit_non_copy_owned_value_source(expression, resolved, environment)
            {
                diagnostics.push(non_copy_struct_return_diagnostic(
                    sources,
                    expression,
                    &source.source_name,
                    &source.type_name,
                    source.kind,
                    context,
                ));
            }
        }
    }
}

pub(super) fn check_borrow_return_provenance(
    sources: &SourceMap,
    expression: &Expr,
    ty: &Type,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(provenance) = borrow_return_provenance_for_expression(
        expression,
        ty,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) else {
        return;
    };
    let Some(source) = provenance.escaping_source() else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources, expression, source, context,
    ));
}

pub(super) fn check_propagated_fallible_error_borrow_return_provenance(
    sources: &SourceMap,
    expression: &PropagationExpr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
    borrow_provenance: &BorrowReturnEnvironment,
    summaries: &BorrowReturnSummaries,
) {
    if !propagated_fallible_error_can_escape(
        &expression.expression,
        &context.declared_type,
        resolved,
        environment,
    ) {
        return;
    }

    let Some(provenance) = borrow_return_fallible_error_provenance_for_expression(
        &expression.expression,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    ) else {
        return;
    };
    let Some(source) = provenance.escaping_source() else {
        return;
    };

    diagnostics.push(borrow_return_escapes_diagnostic(
        sources,
        &expression.expression,
        source,
        context,
    ));
}
