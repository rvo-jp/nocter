use super::*;

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_block_result(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let Some(result) = &block.result else {
        return None;
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
    let result_type = expression_type(result, resolved, &result_environment);
    borrow_return_provenance_for_expression(
        result,
        &result_type,
        resolved,
        &result_environment,
        &result_borrow_provenance,
        summaries,
    )
}

pub(in crate::typecheck::returns) fn apply_borrow_return_statement_effects(
    block: &crate::ast::Block,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    for statement in &block.statements {
        apply_borrow_return_statement_effect(
            statement,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }
    if let Some(result) = &block.result {
        apply_retained_call_mutations(result, resolved, environment, borrow_provenance, summaries);
    }
}

pub(in crate::typecheck::returns) fn apply_borrow_return_statement_effect(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    match statement {
        Stmt::Binding(statement) => {
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
                statement.name_span,
                type_contains_borrow_like(&binding_type, resolved)
                    || provenance
                        .as_ref()
                        .is_some_and(ValueProvenance::has_storage_dependency),
                provenance,
            );
        }
        Stmt::Assignment(statement) => {
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
                if let Some(symbol) = resolved.local_symbol_for_identifier(identifier) {
                    borrow_provenance.define_binding(
                        symbol.name_span,
                        type_contains_borrow_like(target_type, resolved)
                            || provenance
                                .as_ref()
                                .is_some_and(ValueProvenance::has_storage_dependency),
                        provenance,
                    );
                }
            } else if let Some(identifier) = expression_root_identifier(&statement.target) {
                let value_type = expression_type(&statement.value, resolved, environment);
                let value_provenance = borrow_return_provenance_for_expression(
                    &statement.value,
                    &value_type,
                    resolved,
                    environment,
                    borrow_provenance,
                    summaries,
                );
                if let (Some(symbol), Some(value_provenance)) = (
                    resolved.local_symbol_for_identifier(identifier),
                    value_provenance,
                ) {
                    let mut next = borrow_provenance
                        .get(symbol.name_span)
                        .cloned()
                        .unwrap_or_else(|| {
                            if matches!(symbol.kind, LocalSymbolKind::Parameter) {
                                ValueProvenance::input(InputId::declared_at(symbol.name_span))
                            } else {
                                ValueProvenance::Independent
                            }
                        });
                    next.merge(&value_provenance);
                    borrow_provenance.define_binding(symbol.name_span, true, Some(next));
                }
            }
        }
        Stmt::If(statement) => {
            let mut then_environment = environment.clone();
            let mut then_borrow_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::IfIs(statement) => {
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
            apply_borrow_return_statement_effects(
                &statement.then_block,
                resolved,
                &mut then_environment,
                &mut then_borrow_provenance,
                summaries,
            );
            let mut incoming = vec![then_borrow_provenance];
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    else_block,
                    resolved,
                    &mut else_environment,
                    &mut else_borrow_provenance,
                    summaries,
                );
                incoming.push(else_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::Switch(statement) => {
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
                apply_borrow_return_statement_effects(
                    &arm.body,
                    resolved,
                    &mut arm_environment,
                    &mut arm_borrow_provenance,
                    summaries,
                );
                incoming.push(arm_borrow_provenance);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut wildcard_environment = environment.clone();
                let mut wildcard_borrow_provenance = borrow_provenance.clone();
                apply_borrow_return_statement_effects(
                    &wildcard_arm.body,
                    resolved,
                    &mut wildcard_environment,
                    &mut wildcard_borrow_provenance,
                    summaries,
                );
                incoming.push(wildcard_borrow_provenance);
            } else {
                incoming.push(borrow_provenance.clone());
            }
            borrow_provenance.join_reachable(&incoming);
        }
        Stmt::ForRange(statement) => {
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            let initial = borrow_provenance.clone();
            borrow_provenance.join_reachable(&[initial, body_provenance]);
        }
        Stmt::CollectionFor(statement) => {
            let (item_type, item_provenance) = collection_iteration_item_flow(
                statement,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment =
                environment_for_collection_for_binding(statement, item_type.clone(), environment);
            let mut body_provenance = borrow_provenance.clone();
            define_collection_iteration_item_provenance(
                statement,
                &item_type,
                item_provenance,
                resolved,
                &mut body_provenance,
            );
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            let initial = borrow_provenance.clone();
            borrow_provenance.join_reachable(&[initial, body_provenance]);
        }
        Stmt::LiteralPackFor(statement) => {
            let mut body_environment = environment_for_literal_pack_binding(statement, environment);
            let mut body_provenance = borrow_provenance.clone();
            define_literal_pack_item_provenance(
                statement,
                environment,
                resolved,
                &mut body_provenance,
            );
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            let initial = borrow_provenance.clone();
            borrow_provenance.join_reachable(&[initial, body_provenance]);
        }
        Stmt::While(statement) => {
            let mut body_environment = environment.clone();
            let mut body_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            let initial = borrow_provenance.clone();
            borrow_provenance.join_reachable(&[initial, body_provenance]);
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_provenance = borrow_provenance.clone();
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            borrow_provenance.update_existing_from(&body_provenance);
        }
        Stmt::Region(statement) => {
            let mut body_environment = environment.clone();
            body_environment.define(
                statement.name.clone(),
                crate::typecheck::regions::region_binding_type(statement, resolved, environment),
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
            apply_borrow_return_statement_effects(
                &statement.body,
                resolved,
                &mut body_environment,
                &mut body_provenance,
                summaries,
            );
            borrow_provenance.update_existing_from(&body_provenance);
        }
        Stmt::Expression(statement) => apply_retained_call_mutations(
            &statement.expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ),
        _ => {}
    }
}

pub(in crate::typecheck::returns) fn define_if_is_payload_borrow_return_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let Some(binding) = statement
        .payload
        .as_ref()
        .and_then(|payload| payload.binding())
    else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        &statement.expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

pub(in crate::typecheck::returns) fn define_switch_arm_payload_borrow_return_binding(
    arm: &SwitchArm,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let Some(binding) = arm.payload.as_ref().and_then(|payload| payload.binding()) else {
        return;
    };
    define_payload_borrow_return_binding(
        binding,
        target_expression,
        resolved,
        source_environment,
        payload_environment,
        borrow_provenance,
        summaries,
    );
}

pub(in crate::typecheck::returns) fn define_payload_borrow_return_binding(
    binding: &SwitchPayloadBinding,
    target_expression: &Expr,
    resolved: &ResolveOutput,
    source_environment: &TypeEnvironment,
    payload_environment: &TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) {
    let Some(binding_type) = payload_environment.get(&binding.name) else {
        borrow_provenance.define_binding(binding.span, false, None);
        return;
    };
    let contains_borrow_like = type_contains_borrow_like(binding_type, resolved);
    let provenance = contains_borrow_like.then(|| {
        let target_type = expression_type(target_expression, resolved, source_environment);
        borrow_return_provenance_for_expression(
            target_expression,
            &target_type,
            resolved,
            source_environment,
            borrow_provenance,
            summaries,
        )
    });
    borrow_provenance.define_binding(binding.span, contains_borrow_like, provenance.flatten());
}
