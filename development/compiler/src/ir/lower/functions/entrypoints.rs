use super::*;

pub(in crate::ir::lower) fn lower_literal_function<'a>(
    literal: &LiteralDecl,
    specialization: &LiteralSpecialization,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let parameters = literal_lowering_parameters(literal, specialization);
    let name = specialization.target_name.clone();
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, literal.shape_span)
    })?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &specialization.result_type,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, literal.shape_span)
    })?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type = lower_function_return_type(
        &specialization.result_type,
        &name,
        resolved,
        &resolved_sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, literal.return_type.span())
    })?;
    let mut context = LoweringContext::new(
        name.clone(),
        return_type.success_type().clone(),
        function_signatures,
        parameter_slots,
    )
    .with_function_return_type(return_type.clone())
    .with_function_return_type_expr(specialization.result_type.clone())
    .with_function_returns_optional(false)
    .with_call_resolution(
        root_source,
        resolved,
        typed_hir,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(specialization.substitutions.clone())
    .with_error_payloads(error_payloads);
    if let (LiteralShape::Sequence, Some(capture), Some(element_type)) = (
        literal.shape,
        literal.capture.as_ref(),
        specialization.element_type.as_ref(),
    ) {
        context = context.with_literal_pack(LiteralPackLowering {
            capture_name: capture.name.clone(),
            runtime_length_name: specialization
                .pack_segments
                .iter()
                .any(|segment| {
                    matches!(
                        segment,
                        crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Spread { .. }
                    )
                })
                .then(|| {
                    super::super::literal_pack_lengths::runtime_length_name(&capture.name)
                }),
            segments: specialization
                .pack_segments
                .iter()
                .map(|segment| match segment {
                    crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Value {
                        parameter_index,
                    } => LiteralPackLoweringSegment::Value {
                        parameter_name: literal_element_parameter_name(*parameter_index),
                    },
                    crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Spread {
                        iterator_parameter_index,
                        plan,
                    } => LiteralPackLoweringSegment::Spread {
                        iterator_parameter_name: literal_element_parameter_name(
                            *iterator_parameter_index,
                        ),
                        plan: plan.clone(),
                    },
                })
                .collect(),
            element_type: element_type.clone(),
        });
    }

    let mut instructions = parameter_setup;
    instructions.extend(
        super::super::literal_pack_lengths::lower_runtime_length_initialization(&mut context)?,
    );
    instructions.extend(lower_callable_body(
        &name,
        literal
            .body
            .as_ref()
            .expect("literal index contains only body-bearing declarations"),
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);
    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

fn literal_lowering_parameters(
    literal: &LiteralDecl,
    specialization: &LiteralSpecialization,
) -> Vec<Parameter> {
    match specialization.shape {
        LiteralShape::Sequence => specialization
            .argument_types
            .iter()
            .enumerate()
            .map(|(index, ty)| Parameter {
                span: literal
                    .capture
                    .as_ref()
                    .map_or(literal.shape_span, |capture| capture.span),
                name: literal_element_parameter_name(index),
                name_span: literal
                    .capture
                    .as_ref()
                    .map_or(literal.shape_span, |capture| capture.name_span),
                ty: ty.clone(),
            })
            .collect(),
        LiteralShape::String => literal
            .parameters
            .parameters
            .iter()
            .zip(&specialization.argument_types)
            .map(|(parameter, ty)| Parameter {
                span: parameter.span,
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: ty.clone(),
            })
            .collect(),
    }
}

pub(in crate::ir::lower) fn lower_function<'a>(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    mir_bodies: &crate::mir::BodyCache,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    if !function
        .generics
        .parameters
        .iter()
        .all(|parameter| substitutions.contains_key(&parameter.name))
    {
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8007",
                format!(
                    "native lowering can only lower function `{}` with concrete generic arguments, got `{}`",
                    name, function.name
                ),
            )],
            sources,
            function.generics.span.unwrap_or(function.span),
        ));
    }

    let mut contextual_substitutions = substitutions.clone();
    crate::typecheck::extend_associated_type_substitutions_with_resolver(
        &mut contextual_substitutions,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    );
    let parameters = function_parameters(function, &contextual_substitutions);
    let return_type_expr =
        substitute_type_expr_parameters(&function.return_type, &contextual_substitutions);
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type =
        match lower_function_return_type(&return_type_expr, &name, resolved, &resolved_sources) {
            Ok(return_type) => return_type,
            Err(diagnostics) => {
                return Err(attach_primary_span_if_absent(
                    diagnostics,
                    sources,
                    function.return_type.span(),
                ));
            }
        };
    let success_type = return_type.success_type().clone();
    let body = function
        .body
        .as_ref()
        .expect("function index contains only body-bearing declarations");
    if function.generics.parameters.is_empty()
        && let Some(mir_instructions) = super::super::mir::try_lower_body(
            mir_bodies,
            body,
            &parameters,
            &return_type,
            resolved,
            &resolved_sources,
            typed_hir,
            &name,
            &function_signatures,
            &function_names,
            &parameter_slots,
            root_source,
            sources,
        )
    {
        let mut instructions = parameter_setup;
        instructions.extend(mir_instructions?);
        return Ok(Function {
            name,
            target,
            return_type,
            instructions,
        });
    }
    let mut context = LoweringContext::new(
        name.clone(),
        success_type,
        function_signatures,
        parameter_slots,
    )
    .with_function_return_type(return_type.clone())
    .with_function_return_type_expr(return_type_expr.clone())
    .with_function_returns_optional(return_type_expr_has_optional_layer_with_resolver(
        &return_type_expr,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    ))
    .with_call_resolution(
        root_source,
        resolved,
        typed_hir,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(contextual_substitutions)
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(lower_callable_body(
        &function.name,
        body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

pub(in crate::ir::lower) fn lower_drop_function<'a>(
    drop_: &DestructDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let binding = Parameter {
        span: drop_.binding.span,
        name: drop_.binding.name.clone(),
        name_span: drop_.binding.name_span,
        ty: substitute_type_expr_parameters(
            &type_expr_with_self_type(&drop_.binding.ty, self_ty),
            substitutions,
        ),
    };
    let parameters = lower_scalar_parameters(
        &name,
        std::slice::from_ref(&binding),
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, binding.span))?;
    validate_parameter_slots_match_function_abi(
        &name,
        std::slice::from_ref(&binding),
        &void_type_expr(drop_.span),
        resolved,
        &resolved_sources,
        &parameters,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, binding.span))?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameters);
    let return_type = Type::Void;
    let mut context = LoweringContext::new(
        name.clone(),
        return_type.clone(),
        function_signatures,
        parameters,
    )
    .with_function_return_type(return_type.clone())
    .with_function_returns_optional(false)
    .with_call_resolution(
        root_source,
        resolved,
        typed_hir,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(substitutions.clone())
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(lower_callable_body(
        &name,
        &drop_.body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

pub(in crate::ir::lower) fn lower_method_function<'a>(
    method: &crate::ast::CallableDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    lower_method_function_with_prologue(
        method,
        self_ty,
        substitutions,
        name,
        sources,
        target,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        resolved_sources,
        error_payloads,
        |_| Ok(Vec::new()),
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::ir::lower) fn lower_method_function_with_prologue<'a>(
    method: &crate::ast::CallableDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
    prologue: impl FnOnce(&mut LoweringContext<'a>) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Function, Vec<Diagnostic>> {
    let Some(body) = &method.body else {
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8007",
                format!("native lowering can only lower method `{name}` with a body"),
            )],
            sources,
            method.span,
        ));
    };

    let concrete_self_ty = substitute_type_expr_parameters(self_ty, substitutions);
    let mut contextual_substitutions = substitutions.clone();
    contextual_substitutions.insert("Self".to_string(), concrete_self_ty.clone());
    crate::typecheck::extend_associated_type_substitutions_with_resolver(
        &mut contextual_substitutions,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    );
    let parameters = method_parameters(method, &concrete_self_ty, &contextual_substitutions);
    let return_type_expr =
        substitute_type_expr_parameters(&method.return_type, &contextual_substitutions);
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, method.parameters.span)
    })?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, method.span))?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type =
        match lower_function_return_type(&return_type_expr, &name, resolved, &resolved_sources) {
            Ok(return_type) => return_type,
            Err(diagnostics) => {
                return Err(attach_primary_span_if_absent(
                    diagnostics,
                    sources,
                    method.return_type.span(),
                ));
            }
        };
    let success_type = return_type.success_type().clone();
    let mut context = LoweringContext::new(
        name.clone(),
        success_type,
        function_signatures,
        parameter_slots,
    )
    .with_function_return_type(return_type.clone())
    .with_function_return_type_expr(return_type_expr.clone())
    .with_function_returns_optional(return_type_expr_has_optional_layer_with_resolver(
        &return_type_expr,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    ))
    .with_call_resolution(
        root_source,
        resolved,
        typed_hir,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(contextual_substitutions)
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(prologue(&mut context)?);
    instructions.extend(lower_callable_body(
        &name,
        body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}
