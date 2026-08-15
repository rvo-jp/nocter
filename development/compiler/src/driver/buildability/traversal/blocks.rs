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

    if let Some(closure) = &callable.closure_mir
        && let Some((return_representation, return_mode, outcome_layers)) =
            callable_return_contract(callable, resolved_sources)
        && let Some(body_id) = callable.resolved.semantic_db.body_at(callable.body.span)
    {
        let body = mir_bodies.get_or_build_specialized(
            callable.body.span.source,
            body_id,
            &callable.substitutions,
            || {
                crate::mir::try_build_closure_body(
                    closure.expression,
                    &closure.plan.ty,
                    closure.receiver_mode,
                    return_representation,
                    return_mode,
                    crate::mir::BuildInputs {
                        semantic_db: &callable.resolved.semantic_db,
                        resolved: callable.resolved,
                        resolved_sources,
                        typed_hir: callable.typed_hir,
                        declared_return_ty: callable
                            .return_type
                            .as_ref()
                            .and_then(|ty| callable.typed_hir.type_id(ty)),
                        outcome_layers: outcome_layers.clone(),
                    },
                )
            },
        );
        match body {
            Some(Ok(body)) => {
                if let Err(message) = enqueue_mir_call_targets(
                    &body,
                    callable,
                    callable.typed_hir,
                    root_source,
                    names,
                    queue,
                ) {
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

    if let Some(parameters) = callable.mir_parameters
        && let Some((return_representation, return_mode, outcome_layers)) =
            callable_return_contract(callable, resolved_sources)
        && let Some(body_id) = callable.resolved.semantic_db.body_at(callable.body.span)
    {
        let specialized_hir = callable.typed_hir.specialized(&callable.substitutions);
        let specialized_parameters = parameters
            .iter()
            .cloned()
            .map(|mut parameter| {
                parameter.ty = crate::ast::substitute_type_expr_parameters(
                    &parameter.ty,
                    &callable.substitutions,
                );
                parameter
            })
            .collect::<Vec<_>>();
        let body = mir_bodies.get_or_build_specialized(
            callable.body.span.source,
            body_id,
            &callable.substitutions,
            || {
                crate::mir::try_build_body_with_return_mode(
                    callable.body,
                    &specialized_parameters,
                    return_representation,
                    return_mode,
                    crate::mir::BuildInputs {
                        semantic_db: &callable.resolved.semantic_db,
                        resolved: callable.resolved,
                        resolved_sources,
                        typed_hir: &specialized_hir,
                        declared_return_ty: callable
                            .return_type
                            .as_ref()
                            .and_then(|ty| specialized_hir.type_id(ty)),
                        outcome_layers: outcome_layers.clone(),
                    },
                )
            },
        );
        match body {
            Some(Ok(body)) => {
                if let Err(message) = enqueue_mir_call_targets(
                    &body,
                    callable,
                    &specialized_hir,
                    root_source,
                    names,
                    queue,
                ) {
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
    typed_hir: &crate::typecheck::TypedHir,
    root_source: SourceId,
    names: &CallableNames,
    queue: &mut VecDeque<CallTarget>,
) -> Result<(), String> {
    for block in &body.blocks {
        let crate::mir::Terminator::Call {
            ref callee,
            ref continuation,
            ..
        } = block.terminator
        else {
            continue;
        };
        if matches!(callee.callable, crate::mir::CallableIdentity::Intrinsic(_)) {
            continue;
        }
        if matches!(
            continuation,
            crate::mir::CallContinuation::Return { destination, .. }
                if body.locals[destination.local.index()].representation
                    == crate::mir::ValueRepresentation::Error
        ) {
            // Error-returning helper bodies are validated as static payloads and
            // projected at the call site. They have no runtime callable edge.
            continue;
        }
        let name = names
            .get_instance(callee, typed_hir)
            .ok_or_else(|| format!("MIR call target has no indexed runtime name: {callee:?}"))?;
        let source = match &callee.callable {
            crate::mir::CallableIdentity::Intrinsic(_) => unreachable!("handled above"),
            crate::mir::CallableIdentity::Definition(definition)
            | crate::mir::CallableIdentity::Literal { definition, .. } => {
                callable
                    .resolved
                    .semantic_db
                    .definition_anchor(*definition)
                    .ok_or_else(|| "MIR call target has no source anchor".to_string())?
                    .source
            }
            crate::mir::CallableIdentity::Value { ty, .. } => {
                typed_hir
                    .type_expr_by_id(*ty)
                    .ok_or_else(|| "MIR callable-value type is missing".to_string())?
                    .span()
                    .source
            }
        };
        queue.push_back(call_target_for_source(source, root_source, name.clone()));
    }
    Ok(())
}

fn callable_return_contract(
    callable: &IndexedCallable<'_>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(
    crate::mir::ValueRepresentation,
    crate::mir::ReturnMode,
    Vec<crate::outcomes::OutcomeLayer>,
)> {
    let return_type = callable.return_type.as_ref()?;
    let outcome_shape = outcome_shape_with_resolver(return_type, callable.resolved, |source| {
        resolved_sources.get(&source).copied()
    });
    let return_mode = if outcome_shape
        .layers
        .contains(&crate::outcomes::OutcomeLayer::Fallible)
    {
        crate::mir::ReturnMode::Fallible
    } else {
        crate::mir::ReturnMode::Plain
    };
    if matches!(
        &outcome_shape.payload,
        TypeExpr::Reference(reference) if reference.name == "void"
    ) {
        return Some((
            crate::mir::ValueRepresentation::Unit,
            return_mode,
            outcome_shape.layers,
        ));
    }
    let value = abi_value_from_type_expr_with_resolver(
        &outcome_shape.payload,
        callable.resolved,
        |source| resolved_sources.get(&source).copied(),
    )
    .ok()?;
    let representation = match value.ty {
        AbiType::I32 => crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::I32),
        AbiType::U8 => crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::U8),
        AbiType::Usize => crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Usize),
        AbiType::Bool => crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Bool),
        AbiType::I8 => scalar_integer(crate::integer::IntegerType::I8),
        AbiType::I16 => scalar_integer(crate::integer::IntegerType::I16),
        AbiType::U16 => scalar_integer(crate::integer::IntegerType::U16),
        AbiType::U32 => scalar_integer(crate::integer::IntegerType::U32),
        AbiType::I64 => scalar_integer(crate::integer::IntegerType::I64),
        AbiType::U64 => scalar_integer(crate::integer::IntegerType::U64),
        AbiType::Isize => scalar_integer(crate::integer::IntegerType::Isize),
        AbiType::StrView => crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str),
        AbiType::Array { .. } | AbiType::Struct(_) | AbiType::Enum(_) => {
            crate::mir::ValueRepresentation::Aggregate
        }
        _ => return None,
    };
    Some((representation, return_mode, outcome_shape.layers))
}

fn scalar_integer(kind: crate::integer::IntegerType) -> crate::mir::ValueRepresentation {
    crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Integer(kind))
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
