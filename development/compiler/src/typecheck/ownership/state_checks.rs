use super::*;

pub(super) fn check_statement_ownership(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) -> FlowState {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => FlowState::fallthrough(),
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_ownership(
                    sources,
                    expression,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
            FlowState::terminal()
        }
        Stmt::Binding(statement) => {
            check_expression_ownership(
                sources,
                &statement.initializer,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let initializer_reaches_end = initializer_type != Type::Never;
            let mut flow = FlowState::fallthrough();
            if !initializer_reaches_end {
                return FlowState::terminal();
            }
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            ownership.define_binding(
                statement.name.clone(),
                statement.name_span,
                &binding_type,
                resolved,
                environment,
            );
            flow.reaches_end = true;
            flow
        }
        Stmt::Assignment(statement) => {
            check_assignment_target_ownership(
                sources,
                &statement.target,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.value,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );

            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(ty) = environment.get(&identifier.name)
            {
                ownership.define_binding(
                    identifier.name.clone(),
                    identifier.span,
                    ty,
                    resolved,
                    environment,
                );
            }
            if expression_type(&statement.value, resolved, environment) == Type::Never {
                FlowState::terminal()
            } else {
                FlowState::fallthrough()
            }
        }
        Stmt::If(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment = environment.clone();
            let mut then_ownership = ownership.clone();
            let then_flow = check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                summaries,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            let then_reaches_end = then_flow.reaches_end;
            let mut flow = FlowState::from_nested(then_flow);
            let mut incoming = Vec::new();
            if then_reaches_end {
                incoming.push(then_ownership);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                let else_reaches_end = else_flow.reaches_end;
                flow.extend_nested(else_flow);
                if else_reaches_end {
                    incoming.push(else_ownership);
                }
            } else {
                incoming.push(ownership.clone());
            }
            flow.reaches_end = !incoming.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&incoming);
            }
            flow
        }
        Stmt::IfIs(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_ownership = ownership.clone();
            if let Some(payload) = statement
                .payload
                .as_ref()
                .and_then(|payload| payload.binding())
            {
                then_ownership.define_binding_from_environment(
                    &payload.name,
                    payload.span,
                    &then_environment,
                    resolved,
                );
            }
            let then_flow = check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                summaries,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            let then_reaches_end = then_flow.reaches_end;
            let mut flow = FlowState::from_nested(then_flow);
            let mut incoming = Vec::new();
            if then_reaches_end {
                incoming.push(then_ownership);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                let else_reaches_end = else_flow.reaches_end;
                flow.extend_nested(else_flow);
                if else_reaches_end {
                    incoming.push(else_ownership);
                }
            } else {
                incoming.push(ownership.clone());
            }
            flow.reaches_end = !incoming.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&incoming);
            }
            flow
        }
        Stmt::Switch(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );

            let mut flow = FlowState::terminal();
            let mut branch_ownerships = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_ownership = ownership.clone();
                if let Some(payload) = arm.payload.as_ref().and_then(|payload| payload.binding()) {
                    arm_ownership.define_binding_from_environment(
                        &payload.name,
                        payload.span,
                        &arm_environment,
                        resolved,
                    );
                }
                let arm_flow = check_block_ownership(
                    sources,
                    &arm.body,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_ownership,
                );
                if arm_flow.reaches_end {
                    branch_ownerships.push(arm_ownership);
                }
                flow.extend_nested(arm_flow);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    &wildcard_arm.body,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                if else_flow.reaches_end {
                    branch_ownerships.push(else_ownership);
                }
                flow.extend_nested(else_flow);
            } else if !switch_statement_covers_all_variants(statement, resolved, environment) {
                branch_ownerships.push(ownership.clone());
            }
            flow.reaches_end = !branch_ownerships.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&branch_ownerships);
            }
            flow
        }
        Stmt::While(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );

            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::ForRange(statement) => {
            check_expression_ownership(
                sources,
                &statement.start,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.end,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::CollectionFor(statement) => {
            let resolution = super::super::iteration::resolve_collection_iteration(
                statement,
                resolved,
                environment,
            )
            .ok();
            check_expression_ownership(
                sources,
                &statement.source,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            if resolution.as_ref().is_some_and(|plan| {
                plan.source_mode == super::super::iteration::CollectionIterationSourceMode::Direct
            }) && let Expr::Identifier(identifier) = statement.source.without_groups()
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            }
            let item_type = resolution
                .as_ref()
                .map_or(Type::Unknown, |plan| plan.item_type.clone());
            let mut body_environment =
                environment_for_collection_for_binding(statement, item_type, environment);
            let mut body_ownership = ownership.clone();
            let source_loan = resolution
                .as_ref()
                .filter(|plan| {
                    plan.source_mode
                        == super::super::iteration::CollectionIterationSourceMode::ReadonlyConversion
                })
                .and_then(|_| direct_borrow_source(&statement.source))
                .map(|source| ActiveBorrow {
                    source: source.source,
                    borrow_name: format!("collection iterator `{}`", statement.name),
                    borrow_span: source.source_span,
                    is_readwrite: source.is_readwrite,
                    scope_bound: true,
                })
                .into_iter()
                .collect();
            let body_flow = check_block_ownership_with_borrows(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
                source_loan,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::LiteralPackFor(statement) => {
            let mut body_environment = environment_for_literal_pack_binding(statement, environment);
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = body_flow.break_states.clone();
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.continue_states.iter().cloned());
            if incoming.is_empty() {
                FlowState::terminal()
            } else {
                ownership.join_branches(&incoming);
                FlowState::fallthrough()
            }
        }
        Stmt::Region(statement) => {
            check_expression_ownership(
                sources,
                &statement.allocator,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            let mut body_environment = environment.clone();
            body_environment.define(
                statement.name.clone(),
                crate::typecheck::regions::region_binding_type(statement, resolved, environment),
            );
            let mut body_ownership = ownership.clone();
            let flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                summaries,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            if flow.reaches_end {
                *ownership = body_ownership;
            }
            flow
        }
        Stmt::Drop(statement) => {
            let Some(ty) = environment.get(&statement.name) else {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    None,
                ));
                return FlowState::fallthrough();
            };
            if non_copy_owned_type_kind_in_environment(ty, resolved, environment).is_none() {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    Some(ty),
                ));
                return FlowState::fallthrough();
            }
            ownership.ensure_binding_from_environment(
                &statement.name,
                statement.name_span,
                environment,
                resolved,
            );
            ownership.drop_binding(sources, &statement.name, statement.name_span, diagnostics);
            FlowState::fallthrough()
        }
        Stmt::Expression(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            if expression_type(&statement.expression, resolved, environment) == Type::Never {
                FlowState::terminal()
            } else {
                FlowState::fallthrough()
            }
        }
        Stmt::Break(_) => FlowState::break_with(ownership.clone()),
        Stmt::Continue(_) => FlowState::continue_with(ownership.clone()),
    }
}

pub(super) fn check_expression_ownership(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    match expression {
        Expr::Closure(closure) => {
            for capture in &closure.captures {
                let identifier = crate::ast::IdentifierExpr {
                    span: capture.name_span,
                    name: capture.name.clone(),
                };
                ownership.require_initialized(sources, &identifier, "capture", diagnostics);
                if capture.mode == crate::ast::ClosureCaptureMode::Move
                    && let Some(ty) = environment.get(&capture.name)
                    && (non_copy_owned_type_kind_in_environment(ty, resolved, environment)
                        .is_some()
                        || matches!(ty, Type::Parameter(name) if !environment
                            .generic_requirements(name)
                            .is_some_and(|requirements| requirements.has_copy())))
                {
                    ownership.ensure_binding_from_environment(
                        &capture.name,
                        capture.name_span,
                        environment,
                        resolved,
                    );
                    ownership.move_binding(sources, &identifier, diagnostics);
                }
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                check_expression_ownership(
                    sources,
                    element,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
            if let Some(using) = &expression.using {
                check_expression_ownership(
                    sources,
                    &using.allocator,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                check_expression_ownership(
                    sources,
                    &using.allocator,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Identifier(identifier) => {
            ownership.require_initialized(sources, identifier, "use", diagnostics);
        }
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = expression.operand.as_ref()
                && let Some(ty) = environment.get(&identifier.name)
                && (non_copy_owned_type_kind_in_environment(ty, resolved, environment).is_some()
                    || matches!(ty, Type::Parameter(name) if !environment
                        .generic_requirements(name)
                        .is_some_and(|requirements| requirements.has_copy())))
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            }
        }
        Expr::Propagate(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Force(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Catch(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            let success_ownership = ownership.clone();
            let mut catch_environment = environment_for_catch(
                &expression.binding,
                &expression.expression,
                resolved,
                environment,
            );
            let mut catch_ownership = ownership.clone();
            if let crate::ast::CatchBinding::Named { name, span } = &expression.binding {
                catch_ownership.define_binding_from_environment(
                    name,
                    *span,
                    &catch_environment,
                    resolved,
                );
            }
            let catch_flow = check_block_ownership(
                sources,
                &expression.catch_block,
                resolved,
                summaries,
                diagnostics,
                &mut catch_environment,
                &mut catch_ownership,
            );
            if catch_flow.reaches_end {
                ownership.join_branches(&[success_ownership, catch_ownership]);
            }
        }
        Expr::Borrow(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Binary(expression) => {
            check_expression_ownership(
                sources,
                &expression.left,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.right,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Unary(expression) => {
            check_expression_ownership(
                sources,
                &expression.operand,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Call(expression) => {
            if let Some(identifier) =
                consuming_callable_identifier(expression, resolved, environment)
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            } else if let Some(identifier) =
                owned_method_receiver_identifier(expression, resolved, environment)
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            } else if let Some(method) = method_member_for_call(expression)
                && resolved_method_for_call(resolved, expression, environment).is_some()
            {
                check_expression_ownership(
                    sources,
                    &method.object,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            } else {
                check_expression_ownership(
                    sources,
                    &expression.callee,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }

            for argument in &expression.arguments {
                check_expression_ownership(
                    sources,
                    argument,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Index(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.index,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_ownership(
                    sources,
                    element,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_ownership(
                    sources,
                    &field.value,
                    resolved,
                    summaries,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    check_expression_ownership(
                        sources,
                        &part.expression,
                        resolved,
                        summaries,
                        diagnostics,
                        environment,
                        ownership,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            check_expression_ownership(
                sources,
                &expression.value,
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
            let present_ownership = ownership.clone();
            let mut fallback_environment = environment.clone();
            let mut fallback_ownership = ownership.clone();
            let fallback_flow = check_block_ownership(
                sources,
                &expression.fallback,
                resolved,
                summaries,
                diagnostics,
                &mut fallback_environment,
                &mut fallback_ownership,
            );
            let mut incoming = vec![present_ownership];
            if fallback_flow.reaches_end {
                incoming.push(fallback_ownership);
            }
            ownership.join_branches(&incoming);
        }
        Expr::If(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::If((**expression).clone()),
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::IfIs(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::IfIs((**expression).clone()),
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Match(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::Switch((**expression).clone()),
                resolved,
                summaries,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_assignment_target_ownership(
    sources: &SourceMap,
    target: &Expr,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    if whole_identifier(target).is_some() {
        return;
    }
    check_expression_ownership(
        sources,
        target,
        resolved,
        summaries,
        diagnostics,
        environment,
        ownership,
    );
}
