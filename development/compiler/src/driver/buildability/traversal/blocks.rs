use super::*;

pub(in crate::driver::buildability) fn collect_callable_diagnostics(
    callable: &IndexedCallable<'_>,
    sources: &SourceMap,
    mir_bodies: &crate::mir::BodyCache,
    root_source: SourceId,
    names: &CallableNames,
    resolved_sources: &ResolvedSources<'_>,
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
                crate::mir::build_closure_body(
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
            Ok(body) => {
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
            Err(error) => {
                diagnostics.push(mir_build_error_diagnostic(
                    sources,
                    &error,
                    callable.body.span,
                ));
                return;
            }
        }
    }

    if let Some(parameters) = callable.mir_parameters.as_deref()
        && let Some((return_representation, return_mode, outcome_layers)) =
            callable_return_contract(callable, resolved_sources)
        && let Some(body_id) = callable.resolved.semantic_db.body_at(callable.body.span)
    {
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
        let specialized_return_type = crate::ast::substitute_type_expr_parameters(
            callable
                .return_type
                .as_ref()
                .expect("callable return contract requires a return type"),
            &callable.substitutions,
        );
        let literal_pack = callable.literal_pack.clone();
        let specialized_hir = crate::mir::prepare_typed_hir(
            callable.typed_hir,
            &callable.substitutions,
            &specialized_parameters,
            &specialized_return_type,
            literal_pack.as_ref(),
        );
        let build = || {
            let inputs = crate::mir::BuildInputs {
                semantic_db: &callable.resolved.semantic_db,
                resolved: callable.resolved,
                resolved_sources,
                typed_hir: &specialized_hir,
                declared_return_ty: specialized_hir.type_id(&specialized_return_type),
                outcome_layers: outcome_layers.clone(),
            };
            if let Some(literal_pack) = literal_pack.clone() {
                crate::mir::build_literal_body(
                    callable.body,
                    &specialized_parameters,
                    return_representation,
                    return_mode,
                    inputs,
                    literal_pack,
                )
            } else {
                crate::mir::build_body_with_return_mode(
                    callable.body,
                    &specialized_parameters,
                    return_representation,
                    return_mode,
                    inputs,
                )
            }
        };
        let body = if let Some(literal_instance) = callable.literal_instance.clone() {
            mir_bodies.get_or_build_literal_specialized(
                callable.body.span.source,
                body_id,
                &callable.substitutions,
                literal_instance,
                build,
            )
        } else {
            mir_bodies.get_or_build_specialized(
                callable.body.span.source,
                body_id,
                &callable.substitutions,
                build,
            )
        };
        match body {
            Ok(body) => {
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
            Err(error) => {
                diagnostics.push(mir_build_error_diagnostic(
                    sources,
                    &error,
                    callable.body.span,
                ));
                return;
            }
        }
    }

    diagnostics.push(
        Diagnostic::error(
            "E8000",
            "compiler could not identify a checked MIR body for this callable",
        )
        .with_primary_span_if_absent(sources, callable.body.span),
    );
}

fn mir_build_error_diagnostic(
    sources: &SourceMap,
    error: &crate::mir::BuildError,
    fallback_span: ByteSpan,
) -> Diagnostic {
    let mut root = error;
    while let crate::mir::BuildError::Context { source, .. }
    | crate::mir::BuildError::ClosureBody(source) = root
    {
        root = source;
    }
    match root {
        crate::mir::BuildError::UnloadedImportedCall { span, path } => {
            unsupported_native_build_diagnostic(
                sources,
                *span,
                "unloaded imported function calls",
                &format!(
                    "load `{path}` from the active Nocter home or use a same-module function until imported placeholder lowering is promoted"
                ),
            )
        }
        crate::mir::BuildError::UnspecializedGenericCall { span } => {
            unsupported_native_build_diagnostic(
                sources,
                *span,
                "generic function calls without concrete type arguments",
                "make every generic parameter concrete through argument types or return context",
            )
        }
        crate::mir::BuildError::UnsupportedSource {
            span,
            construct,
            help,
        } => unsupported_native_build_diagnostic(sources, *span, construct, help),
        crate::mir::BuildError::UnsupportedClaimedExpression => {
            unsupported_native_build_diagnostic(
                sources,
                fallback_span,
                "this checked source form through MIR",
                "rewrite this operation using a source form with a complete native MIR representation",
            )
        }
        _ => Diagnostic::error(
            "E8000",
            format!("compiler could not construct MIR: {error:?}"),
        )
        .with_primary_span_if_absent(sources, fallback_span),
    }
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
        let crate::mir::Terminator::Call { ref callee, .. } = block.terminator else {
            continue;
        };
        if matches!(callee.callable, crate::mir::CallableIdentity::Intrinsic(_)) {
            continue;
        }
        let name = names.get_instance(callee, typed_hir).ok_or_else(|| {
            let receiver = callee
                .receiver
                .and_then(|ty| typed_hir.type_expr_by_id(ty))
                .map(crate::ast::canonical_type_expr);
            format!(
                "MIR call target has no indexed runtime name: {callee:?}, receiver={receiver:?}"
            )
        })?;
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
        TypeExpr::Reference(reference) if matches!(reference.name.as_str(), "void" | "never")
    ) {
        return Some((
            crate::mir::ValueRepresentation::Unit,
            return_mode,
            outcome_shape.layers,
        ));
    }
    if type_expr_is_error_parameter_with_resolver(
        &outcome_shape.payload,
        callable.resolved,
        &|source| resolved_sources.get(&source).copied(),
    ) {
        return Some((
            crate::mir::ValueRepresentation::Error,
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
        AbiType::Usize | AbiType::Pointer => {
            crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Usize)
        }
        AbiType::Bool => crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::Bool),
        AbiType::I8 => scalar_integer(crate::integer::IntegerType::I8),
        AbiType::I16 => scalar_integer(crate::integer::IntegerType::I16),
        AbiType::U16 => scalar_integer(crate::integer::IntegerType::U16),
        AbiType::U32 => scalar_integer(crate::integer::IntegerType::U32),
        AbiType::I64 => scalar_integer(crate::integer::IntegerType::I64),
        AbiType::U64 => scalar_integer(crate::integer::IntegerType::U64),
        AbiType::Isize => scalar_integer(crate::integer::IntegerType::Isize),
        AbiType::StrView => crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str),
        AbiType::SliceView => crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice),
        AbiType::Borrow => crate::mir::ValueRepresentation::Borrow,
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

fn enqueue_drop_targets_in_callable(
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
