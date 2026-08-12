use super::*;

pub(in crate::typecheck) fn check_ownership_states(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let Some(body) = &function.body else {
                    continue;
                };
                let mut environment = environment_for_function(function, resolved);
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    &function.parameters.parameters,
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    body,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            Item::Test(test) => {
                let mut environment = TypeEnvironment::default();
                let mut ownership = OwnershipState::default();
                check_block_ownership(
                    sources,
                    &test.body,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            Item::Instance(instance) => {
                check_instance_member_ownership(sources, instance, resolved, summaries, diagnostics)
            }
            Item::Destruct(destruct) => {
                let mut environment = environment_for_destruct(destruct, resolved);
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    std::slice::from_ref(&destruct.binding),
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &destruct.body,
                    resolved,
                    summaries,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            Item::Conformance(conformance) => check_conformance_member_ownership(
                sources,
                conformance,
                resolved,
                summaries,
                diagnostics,
            ),
            Item::Interface(interface) => {
                for method in &interface.methods {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut environment =
                        environment_for_interface_method(method, resolved, interface);
                    let mut ownership = OwnershipState::default();
                    ownership.define_binding_from_environment(
                        &method.receiver.name,
                        method.receiver.name_span,
                        &environment,
                        resolved,
                    );
                    ownership.define_parameters(
                        &method.parameters.parameters,
                        &environment,
                        resolved,
                    );
                    check_block_ownership(
                        sources,
                        body,
                        resolved,
                        summaries,
                        diagnostics,
                        &mut environment,
                        &mut ownership,
                    );
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    check_function_ownership(sources, function, resolved, summaries, diagnostics);
                }
                for (_, literal) in construct.literals() {
                    check_literal_ownership(sources, literal, resolved, summaries, diagnostics);
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn check_function_ownership(
    sources: &SourceMap,
    function: &crate::ast::FunctionDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = &function.body else {
        return;
    };
    let mut environment = environment_for_function(function, resolved);
    let mut ownership = OwnershipState::default();
    ownership.define_parameters(&function.parameters.parameters, &environment, resolved);
    check_block_ownership(
        sources,
        body,
        resolved,
        summaries,
        diagnostics,
        &mut environment,
        &mut ownership,
    );
}

fn check_literal_ownership(
    sources: &SourceMap,
    literal: &crate::ast::LiteralDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = &literal.body else {
        return;
    };
    let mut environment = environment_for_literal(literal, resolved);
    let mut ownership = OwnershipState::default();
    ownership.define_parameters(&literal.parameters.parameters, &environment, resolved);
    check_block_ownership(
        sources,
        body,
        resolved,
        summaries,
        diagnostics,
        &mut environment,
        &mut ownership,
    );
}

fn check_method_ownership(
    sources: &SourceMap,
    owner: &(impl MethodOwnerDecl + ?Sized),
    method: &crate::ast::CallableDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = &method.body else { return };
    let mut environment = environment_for_method(method, resolved, owner);
    let mut ownership = OwnershipState::default();
    ownership.define_binding_from_environment(
        &method.receiver.name,
        method.receiver.name_span,
        &environment,
        resolved,
    );
    ownership.define_parameters(&method.parameters.parameters, &environment, resolved);
    check_block_ownership(
        sources,
        body,
        resolved,
        summaries,
        diagnostics,
        &mut environment,
        &mut ownership,
    );
}

fn check_instance_member_ownership(
    sources: &SourceMap,
    instance: &InstanceDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in instance.callables() {
        check_method_ownership(sources, instance, method, resolved, summaries, diagnostics)
    }
}

fn check_conformance_member_ownership(
    sources: &SourceMap,
    conformance: &ConformanceDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &conformance.members {
        if let ConformanceMember::Method(method) = member {
            check_method_ownership(
                sources,
                conformance,
                method,
                resolved,
                summaries,
                diagnostics,
            );
        }
    }
}

pub(super) fn check_block_ownership(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) -> FlowState {
    check_block_ownership_with_borrows(
        sources,
        block,
        resolved,
        summaries,
        diagnostics,
        environment,
        ownership,
        Vec::new(),
    )
}

pub(super) fn check_block_ownership_with_borrows(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
    mut active_borrows: Vec<ActiveBorrow>,
) -> FlowState {
    for (index, statement) in block.statements.iter().enumerate() {
        active_borrows.retain(|borrow| {
            borrow.scope_bound
                || statements_or_result_use_identifier_before_terminal(
                    &block.statements[index..],
                    block.result.as_deref(),
                    &borrow.borrow_name,
                    resolved,
                    environment,
                )
        });
        check_statement_borrow_conflicts(
            sources,
            statement,
            resolved,
            environment,
            &active_borrows,
            diagnostics,
        );

        let flow = check_statement_ownership(
            sources,
            statement,
            resolved,
            summaries,
            diagnostics,
            environment,
            ownership,
        );
        record_statement_borrow(
            statement,
            &block.statements[index + 1..],
            block.result.as_deref(),
            resolved,
            environment,
            summaries,
            &mut active_borrows,
        );
        if !flow.reaches_end {
            return flow;
        }
    }
    if let Some(result) = &block.result {
        active_borrows.retain(|borrow| {
            borrow.scope_bound
                || expression_uses_identifier(result, &borrow.borrow_name, resolved, environment)
        });
        check_expression_borrow_conflicts(
            sources,
            result,
            resolved,
            environment,
            &active_borrows,
            diagnostics,
        );
        check_expression_ownership(
            sources,
            result,
            resolved,
            summaries,
            diagnostics,
            environment,
            ownership,
        );
        if expression_type(result, resolved, environment) == Type::Never {
            return FlowState::terminal();
        }
    }
    FlowState::fallthrough()
}
