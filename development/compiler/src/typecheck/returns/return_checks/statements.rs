use super::*;

pub(in crate::typecheck::returns) fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    borrow_provenance: &mut ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
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
                statement.name_span,
                type_contains_borrow_like(&binding_type, resolved)
                    || provenance
                        .as_ref()
                        .is_some_and(ValueProvenance::has_storage_dependency),
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
        Stmt::CollectionFor(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.source,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let item_type = super::super::super::iteration::resolve_collection_iteration(
                statement,
                resolved,
                environment,
            )
            .map_or(Type::Unknown, |plan| plan.item_type);
            let mut body_environment =
                environment_for_collection_for_binding(statement, item_type, environment);
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
        Stmt::LiteralPackFor(statement) => {
            let mut body_environment = environment_for_literal_pack_binding(statement, environment);
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
        Stmt::Region(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.allocator,
                context,
                resolved,
                diagnostics,
                environment,
                borrow_provenance,
                summaries,
            );
            let mut body_environment = environment.clone();
            body_environment.define(
                statement.name.clone(),
                crate::typecheck::regions::region_binding_type(statement, resolved, environment),
            );
            let mut body_borrow_provenance = borrow_provenance.clone();
            body_borrow_provenance.enter_region(
                crate::typecheck::regions::region_id(statement),
                format!("region `{}`", statement.name),
            );
            body_borrow_provenance.define_binding(
                statement.name_span,
                true,
                Some(ValueProvenance::region(
                    crate::typecheck::regions::region_id(statement),
                    format!("region `{}`", statement.name),
                )),
            );
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
            let region_id = crate::typecheck::regions::region_id(statement);
            if let Some((binding_span, description)) = borrow_provenance
                .first_existing_binding_with_region(&body_borrow_provenance, region_id)
            {
                diagnostics.push(region_binding_escape_diagnostic(
                    sources,
                    statement.span,
                    binding_span,
                    description,
                    region_id.declaration_span(),
                ));
            }
            borrow_provenance.update_existing_from(&body_borrow_provenance);
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
            apply_retained_call_mutations(
                &statement.expression,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
        }
    }
}
