use super::*;

pub(in crate::typecheck::returns) fn collect_return_expression_provenance(
    expression: &Expr,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    collect_expression_fallible_propagation_provenance(
        expression,
        return_type,
        resolved,
        environment,
        borrow_provenance,
        summaries,
        flow,
    );

    let actual = expression_type(expression, resolved, environment);
    let provenance = borrow_return_provenance_for_expression(
        expression,
        &actual,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    );
    if expression_is_fallible_failure_for_return_type(
        expression,
        &actual,
        return_type,
        resolved,
        environment,
    ) {
        flow.merge_fallible_error(provenance);
    } else {
        flow.merge_value(provenance);
    }
}

pub(in crate::typecheck::returns) fn collect_block_result_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    let Some(result) = &block.result else {
        return;
    };
    let mut result_environment = environment.clone();
    let mut result_borrow_provenance = borrow_provenance.clone();
    apply_borrow_return_statement_effects(
        block,
        resolved,
        &mut result_environment,
        &mut result_borrow_provenance,
        summaries,
    );
    collect_return_expression_provenance(
        result,
        return_type,
        resolved,
        &result_environment,
        &result_borrow_provenance,
        summaries,
        flow,
    );
}

pub(in crate::typecheck::returns) fn collect_statement_fallible_propagation_provenance(
    statement: &Stmt,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_fallible_propagation_provenance(
                    expression,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.initializer,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Assignment(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.target,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &statement.value,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::If(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::IfIs(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Switch(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::While(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::ForRange(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.start,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &statement.end,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::CollectionFor(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.source,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::LiteralPackFor(_) => {}
        Stmt::Loop(_) => {}
        Stmt::Region(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.allocator,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Stmt::Expression(statement) => {
            collect_expression_fallible_propagation_provenance(
                &statement.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
    }
}

pub(in crate::typecheck::returns) fn collect_expression_fallible_propagation_provenance(
    expression: &Expr,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    match expression {
        // A closure body propagates through its generated call target, not the
        // enclosing callable being summarized here.
        Expr::Closure(_) => {}
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_fallible_propagation_provenance(
                    element,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
            if let Some(using) = &expression.using {
                collect_expression_fallible_propagation_provenance(
                    &using.allocator,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_fallible_propagation_provenance(
                    &using.allocator,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::Propagate(propagation) => {
            if propagated_fallible_error_can_escape(
                &propagation.expression,
                return_type,
                resolved,
                environment,
            ) {
                flow.merge_fallible_error(borrow_return_fallible_error_provenance_for_expression(
                    &propagation.expression,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                ));
            }
            collect_expression_fallible_propagation_provenance(
                &propagation.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Catch(catch) => {
            collect_expression_fallible_propagation_provenance(
                &catch.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &catch.catch_block,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Force(force) => collect_expression_fallible_propagation_provenance(
            &force.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Borrow(borrow) => collect_expression_fallible_propagation_provenance(
            &borrow.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Unary(unary) => collect_expression_fallible_propagation_provenance(
            &unary.operand,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Binary(binary) => {
            collect_expression_fallible_propagation_provenance(
                &binary.left,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &binary.right,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::TypeConversion(conversion) => collect_expression_fallible_propagation_provenance(
            &conversion.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Call(call) => {
            collect_expression_fallible_propagation_provenance(
                &call.callee,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            for argument in &call.arguments {
                collect_expression_fallible_propagation_provenance(
                    argument,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::Member(member) => collect_expression_fallible_propagation_provenance(
            &member.object,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Index(index) => {
            collect_expression_fallible_propagation_provenance(
                &index.object,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_expression_fallible_propagation_provenance(
                &index.index,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::Group(group) => collect_expression_fallible_propagation_provenance(
            &group.expression,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        ),
        Expr::Otherwise(otherwise) => {
            collect_expression_fallible_propagation_provenance(
                &otherwise.value,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &otherwise.fallback,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
        }
        Expr::If(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.condition,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            collect_block_fallible_propagation_provenance(
                &expression.then_block,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_fallible_propagation_provenance(
                    else_block,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
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
            collect_block_fallible_propagation_provenance(
                &expression.then_block,
                return_type,
                resolved,
                &then_environment,
                &then_borrow_provenance,
                summaries,
                flow,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_fallible_propagation_provenance(
                    else_block,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::Match(expression) => {
            collect_expression_fallible_propagation_provenance(
                &expression.expression,
                return_type,
                resolved,
                environment,
                borrow_provenance,
                summaries,
                flow,
            );
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
                collect_block_fallible_propagation_provenance(
                    &arm.body,
                    return_type,
                    resolved,
                    &arm_environment,
                    &arm_borrow_provenance,
                    summaries,
                    flow,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_block_fallible_propagation_provenance(
                    &wildcard_arm.body,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::ArrayLiteral(literal) => {
            for element in &literal.elements {
                collect_expression_fallible_propagation_provenance(
                    element,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::StructLiteral(literal) => {
            for field in &literal.fields {
                collect_expression_fallible_propagation_provenance(
                    &field.value,
                    return_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                    flow,
                );
            }
        }
        Expr::InterpolatedString(interpolated) => {
            for part in &interpolated.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_fallible_propagation_provenance(
                        &part.expression,
                        return_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                        flow,
                    );
                }
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

pub(in crate::typecheck::returns) fn collect_block_fallible_propagation_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    let mut block_environment = environment.clone();
    let mut block_borrow_provenance = borrow_provenance.clone();
    for statement in &block.statements {
        collect_statement_fallible_propagation_provenance(
            statement,
            return_type,
            resolved,
            &block_environment,
            &block_borrow_provenance,
            summaries,
            flow,
        );
        apply_borrow_return_statement_effect(
            statement,
            resolved,
            &mut block_environment,
            &mut block_borrow_provenance,
            summaries,
        );
        if statement_guarantees_return_or_never(statement, resolved, &block_environment) {
            return;
        }
    }
    if let Some(result) = &block.result {
        collect_expression_fallible_propagation_provenance(
            result,
            return_type,
            resolved,
            &block_environment,
            &block_borrow_provenance,
            summaries,
            flow,
        );
    }
}

pub(in crate::typecheck::returns) fn collect_return_statement_provenance(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
    flow: &mut ProvenanceFlow,
) {
    for statement in &block.statements {
        collect_statement_fallible_propagation_provenance(
            statement,
            return_type,
            resolved,
            environment,
            borrow_provenance,
            summaries,
            flow,
        );
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    collect_return_expression_provenance(
                        expression,
                        return_type,
                        resolved,
                        environment,
                        borrow_provenance,
                        summaries,
                        flow,
                    );
                }
            }
            Stmt::If(if_statement) => {
                let mut then_environment = environment.clone();
                let mut then_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &if_statement.then_block,
                    return_type,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    flow,
                );
                if let Some(else_block) = &if_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        return_type,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::IfIs(if_is_statement) => {
                let mut then_environment =
                    environment_for_if_is_binding(if_is_statement, resolved, environment);
                let mut then_borrow_provenance = borrow_provenance.clone();
                define_if_is_payload_borrow_return_binding(
                    if_is_statement,
                    resolved,
                    environment,
                    &then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                );
                collect_return_statement_provenance(
                    &if_is_statement.then_block,
                    return_type,
                    resolved,
                    &mut then_environment,
                    &mut then_borrow_provenance,
                    summaries,
                    flow,
                );
                if let Some(else_block) = &if_is_statement.else_block {
                    let mut else_environment = environment.clone();
                    let mut else_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        else_block,
                        return_type,
                        resolved,
                        &mut else_environment,
                        &mut else_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::Switch(switch_statement) => {
                for arm in &switch_statement.arms {
                    let mut arm_environment = environment_for_switch_arm(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                    );
                    let mut arm_borrow_provenance = borrow_provenance.clone();
                    define_switch_arm_payload_borrow_return_binding(
                        arm,
                        &switch_statement.expression,
                        resolved,
                        environment,
                        &arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                    );
                    collect_return_statement_provenance(
                        &arm.body,
                        return_type,
                        resolved,
                        &mut arm_environment,
                        &mut arm_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                if let Some(wildcard_arm) = &switch_statement.wildcard_arm {
                    let mut wildcard_environment = environment.clone();
                    let mut wildcard_borrow_provenance = borrow_provenance.clone();
                    collect_return_statement_provenance(
                        &wildcard_arm.body,
                        return_type,
                        resolved,
                        &mut wildcard_environment,
                        &mut wildcard_borrow_provenance,
                        summaries,
                        flow,
                    );
                }
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::While(while_statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &while_statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::ForRange(for_range_statement) => {
                let mut body_environment =
                    environment_for_for_range_binding(for_range_statement, resolved, environment);
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &for_range_statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::CollectionFor(collection_statement) => {
                let item_type = super::super::super::iteration::resolve_collection_iteration(
                    collection_statement,
                    resolved,
                    environment,
                )
                .map_or(Type::Unknown, |plan| plan.item_type);
                let mut body_environment = environment_for_collection_for_binding(
                    collection_statement,
                    item_type,
                    environment,
                );
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &collection_statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::LiteralPackFor(pack_statement) => {
                let mut body_environment =
                    environment_for_literal_pack_binding(pack_statement, environment);
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &pack_statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::Loop(loop_statement) => {
                let mut body_environment = environment.clone();
                let mut body_borrow_provenance = borrow_provenance.clone();
                collect_return_statement_provenance(
                    &loop_statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_borrow_provenance,
                    summaries,
                    flow,
                );
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
            Stmt::Region(statement) => {
                let mut body_environment = environment.clone();
                body_environment.define(
                    statement.name.clone(),
                    crate::typecheck::regions::region_binding_type(
                        statement,
                        resolved,
                        environment,
                    ),
                );
                let mut body_provenance = borrow_provenance.clone();
                body_provenance.enter_region(
                    crate::typecheck::regions::region_id(statement),
                    format!("region `{}`", statement.name),
                );
                body_provenance.define_binding(
                    statement.name_span,
                    true,
                    Some(ValueProvenance::region(
                        crate::typecheck::regions::region_id(statement),
                        format!("region `{}`", statement.name),
                    )),
                );
                collect_return_statement_provenance(
                    &statement.body,
                    return_type,
                    resolved,
                    &mut body_environment,
                    &mut body_provenance,
                    summaries,
                    flow,
                );
                borrow_provenance.update_existing_from(&body_provenance);
            }
            _ => {
                apply_borrow_return_statement_effect(
                    statement,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
            }
        }
        if statement_guarantees_return_or_never(statement, resolved, environment) {
            return;
        }
    }
}
