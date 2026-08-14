use super::*;

pub(in crate::driver::buildability) fn collect_callable_diagnostics(
    callable: &IndexedCallable<'_>,
    sources: &SourceMap,
    mir_bodies: &crate::mir::BodyCache,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for issue in &callable.issues {
        diagnostics.push(unsupported_native_build_diagnostic(
            sources,
            issue.span,
            issue.construct,
            issue.help,
        ));
    }

    enqueue_drop_targets_in_callable(callable, root_source, queue);

    if let Some(parameters) = callable.mir_parameters
        && let Some((return_scalar, return_mode)) =
            callable_scalar_return(callable, resolved_sources)
        && let Some(body_id) = callable.resolved.semantic_db.body_at(callable.body.span)
    {
        let body = mir_bodies.get_or_build(body_id, || {
            crate::mir::try_build_scalar_body_with_return_mode(
                callable.body,
                parameters,
                return_scalar,
                return_mode,
                crate::mir::BuildInputs {
                    semantic_db: &callable.resolved.semantic_db,
                    resolved: callable.resolved,
                    resolved_sources,
                    typed_hir: callable.typed_hir,
                },
            )
        });
        match body {
            Some(Ok(body)) => {
                if let Err(message) =
                    enqueue_mir_call_targets(&body, callable, root_source, names, queue)
                {
                    diagnostics.push(
                        Diagnostic::error("E8000", message)
                            .with_primary_span_if_absent(sources, callable.body.span),
                    );
                }
                return;
            }
            Some(Err(error)) => {
                diagnostics.push(
                    Diagnostic::error(
                        "E8000",
                        format!("compiler could not construct MIR: {error:?}"),
                    )
                    .with_primary_span_if_absent(sources, callable.body.span),
                );
                return;
            }
            None => {}
        }
    }

    collect_terminal_return_block_diagnostics(
        callable.body,
        callable.return_type.as_ref(),
        sources,
        callable.resolved,
        callable.typed_hir,
        &callable.substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

fn enqueue_mir_call_targets(
    body: &crate::mir::Body,
    callable: &IndexedCallable<'_>,
    root_source: SourceId,
    names: &CallableNames,
    queue: &mut VecDeque<CallTarget>,
) -> Result<(), String> {
    for block in &body.blocks {
        let crate::mir::Terminator::Call { ref callee, .. } = block.terminator else {
            continue;
        };
        let name = names
            .get_instance(callee, callable.typed_hir)
            .ok_or_else(|| "MIR call target has no indexed runtime name".to_string())?;
        let source = match callee.callable {
            crate::mir::CallableIdentity::Definition(definition) => {
                callable
                    .resolved
                    .semantic_db
                    .definition_anchor(definition)
                    .ok_or_else(|| "MIR call target has no source anchor".to_string())?
                    .source
            }
            crate::mir::CallableIdentity::Value { ty, .. } => {
                callable
                    .typed_hir
                    .type_expr_by_id(ty)
                    .ok_or_else(|| "MIR callable-value type is missing".to_string())?
                    .span()
                    .source
            }
        };
        queue.push_back(call_target_for_source(source, root_source, name.clone()));
    }
    Ok(())
}

fn callable_scalar_return(
    callable: &IndexedCallable<'_>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(crate::mir::ScalarType, crate::mir::ReturnMode)> {
    let return_type = callable.return_type.as_ref()?;
    let outcome_shape = outcome_shape_with_resolver(return_type, callable.resolved, |source| {
        resolved_sources.get(&source).copied()
    });
    let return_mode = match outcome_shape.layers.as_slice() {
        [] => crate::mir::ReturnMode::Plain,
        [crate::outcomes::OutcomeLayer::Fallible] => crate::mir::ReturnMode::Fallible,
        _ => return None,
    };
    let value = abi_value_from_type_expr_with_resolver(return_type, callable.resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .ok()?;
    let scalar = match value.ty {
        AbiType::I32 => crate::mir::ScalarType::I32,
        AbiType::Usize => crate::mir::ScalarType::Usize,
        AbiType::Bool => crate::mir::ScalarType::Bool,
        _ => return None,
    };
    Some((scalar, return_mode))
}

pub(in crate::driver::buildability) fn enqueue_drop_targets_in_callable(
    callable: &IndexedCallable<'_>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    for specialization in callable.typed_hir.drop_type_specializations() {
        if !span_contains(callable.span, specialization.self_ty.span()) {
            continue;
        }
        let Some(specialization) =
            specialization.with_context_substitutions(&callable.substitutions)
        else {
            continue;
        };
        queue.push_back(call_target_for_source(
            specialization.declaration_span.source,
            root_source,
            specialization.target_name,
        ));
    }
}

pub(in crate::driver::buildability) fn collect_terminal_return_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typed_hir,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = result {
        collect_terminal_return_expression_diagnostics(
            result,
            return_type,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

pub(in crate::driver::buildability) fn collect_block_diagnostics(
    block: &Block,
    return_type: Option<&TypeExpr>,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (statements, result) = reachable_block_parts_for_buildability(
        &block.statements,
        block.result.as_deref(),
        resolved,
        typed_hir,
        generic_substitutions,
    );

    for statement in statements {
        collect_statement_diagnostics(
            statement,
            return_type,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
    if let Some(result) = result {
        collect_expression_diagnostics(
            result,
            sources,
            resolved,
            typed_hir,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

pub(in crate::driver::buildability) fn reachable_block_parts_for_buildability<'a>(
    statements: &'a [Stmt],
    result: Option<&'a Expr>,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> (&'a [Stmt], Option<&'a Expr>) {
    for (index, statement) in statements.iter().enumerate() {
        if statement_exits_function_for_buildability(
            statement,
            resolved,
            typed_hir,
            generic_substitutions,
        ) {
            return (&statements[..=index], None);
        }
    }

    (statements, result)
}
