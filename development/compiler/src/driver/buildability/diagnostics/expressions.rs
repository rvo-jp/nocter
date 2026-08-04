use super::*;

pub(in crate::driver::buildability) fn collect_expression_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Closure(closure) => {
            collect_block_diagnostics(
                &closure.body,
                closure.return_type.as_ref(),
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::InterpolatedString(expression) => {
            if let Some(plan) = typecheck_facts.interpolation_plan(expression.span) {
                queue.push_back(call_target_for_source(
                    plan.constructor.declaration.source,
                    root_source,
                    plan.constructor.target_name.clone(),
                ));
                for part in &plan.parts {
                    queue.push_back(call_target_for_source(
                        part.formatter.declaration.source,
                        root_source,
                        part.formatter.target_name.clone(),
                    ));
                }
            }
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_expression_diagnostics(
                        &part.expression,
                        sources,
                        resolved,
                        typecheck_facts,
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
        }
        Expr::ArrayLiteral(expression) => {
            diagnostics.push(unsupported_native_build_diagnostic(
                sources,
                expression.span,
                "array literals",
                "use scalar/view values or a std collection API once v0 array storage is promoted",
            ));
            for element in &expression.elements {
                collect_expression_diagnostics(
                    element,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_diagnostics(
                    element,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(using) = &expression.using {
                collect_expression_diagnostics(
                    &using.allocator,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_diagnostics(
                    &using.allocator,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                if fixed_array_literal_struct_field_has_fixed_array_type(
                    field,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_fixed_array_literal_elements_diagnostics(
                        unwrap_group_expr(&field.value),
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if let Some(field_type) = otherwise_aggregate_struct_field_type(
                    field,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    let Expr::Otherwise(otherwise) = unwrap_group_expr(&field.value) else {
                        unreachable!("aggregate otherwise field helper checked expression shape");
                    };
                    collect_otherwise_aggregate_value_expression_diagnostics(
                        otherwise,
                        &field_type,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if struct_literal_field_may_use_value_control_expression(
                    field.name_span,
                    typecheck_facts,
                ) {
                    collect_value_expression_diagnostics(
                        &field.value,
                        None,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else {
                    collect_expression_diagnostics(
                        &field.value,
                        sources,
                        resolved,
                        typecheck_facts,
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
        }
        Expr::Propagate(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Force(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Catch(expression) => {
            if !catch_block_runtime_shape_is_buildable(
                &expression.catch_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(unsupported_native_build_diagnostic(
                    sources,
                    expression.catch_block.span,
                    "`catch` blocks outside supported runtime control flow",
                    "end runtime-shipped `catch` blocks with a direct `return` or supported effect-only/never expression statement until broader catch control-flow lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.catch_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Unary(expression) => collect_expression_diagnostics(
            &expression.operand,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Binary(expression) => {
            if binary_uses_storage_only_scalar_value(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                resolved_sources,
            ) {
                diagnostics.push(unsupported_native_build_diagnostic(
                    sources,
                    expression.operator_span,
                    "operations on storage-only scalar values",
                    "use `i32`, `u8`, or `usize` for computed integer values; keep narrow and wide storage-only integers inside aggregate fields",
                ));
            }
            collect_expression_diagnostics(
                &expression.left,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.right,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::TypeConversion(expression) => {
            if conversion_uses_computed_storage_only_scalar_value(
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                resolved_sources,
            ) {
                diagnostics.push(unsupported_native_build_diagnostic(
                    sources,
                    expression.as_span,
                    "conversions from computed storage-only scalar values",
                    "use `i32`, `u8`, or `usize` before computation; storage-only integers are currently supported only as aggregate field values",
                ));
            }
            if conversion_stores_computed_value_in_storage_only_scalar(
                expression,
                resolved,
                generic_substitutions,
                resolved_sources,
            ) {
                diagnostics.push(unsupported_native_build_diagnostic(
                    sources,
                    expression.as_span,
                    "computed values converted to storage-only scalar types",
                    "store an integer literal in the aggregate field, or keep computed values as `i32`, `u8`, or `usize` until broader storage-only scalar lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Call(expression) => {
            let unsupported_std_vec_element_call = unsupported_std_vec_element_call_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                nocter_home,
            );
            if let Some(diagnostic) = &unsupported_std_vec_element_call {
                diagnostics.push(diagnostic.clone());
            }
            if let Some(diagnostic) = unsupported_null_from_addr_call_diagnostic(
                sources,
                expression,
                resolved,
                nocter_home,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) =
                unsupported_unloaded_imported_call_diagnostic(sources, expression, resolved)
            {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_borrow_call_argument_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_method_borrow_receiver_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_unspecialized_generic_function_call_diagnostic(
                sources,
                expression,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_unspecialized_generic_method_call_diagnostic(
                sources,
                expression,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if !payload_enum_constructor_call_is_supported(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                collect_expression_diagnostics(
                    &expression.callee,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if unsupported_std_vec_element_call.is_none()
                && let Some(target) = call_target_for_call(
                    expression,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                )
            {
                queue.push_back(target);
            }
            for (index, argument) in expression.arguments.iter().enumerate() {
                if fixed_array_literal_argument_has_fixed_array_parameter_type(
                    expression,
                    index,
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_fixed_array_literal_elements_diagnostics(
                        unwrap_group_expr(argument),
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if let Some(parameter_type) = otherwise_aggregate_argument_parameter_type(
                    expression,
                    index,
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    let Expr::Otherwise(otherwise) = unwrap_group_expr(argument) else {
                        unreachable!(
                            "aggregate otherwise argument helper checked expression shape"
                        );
                    };
                    collect_otherwise_aggregate_value_expression_diagnostics(
                        otherwise,
                        &parameter_type,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else if call_argument_may_use_value_control_expression(
                    expression,
                    index,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                ) {
                    collect_value_expression_diagnostics(
                        argument,
                        None,
                        sources,
                        resolved,
                        typecheck_facts,
                        generic_substitutions,
                        root_source,
                        names,
                        resolved_sources,
                        nocter_home,
                        queue,
                        diagnostics,
                    );
                } else {
                    collect_expression_diagnostics(
                        argument,
                        sources,
                        resolved,
                        typecheck_facts,
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
        }
        Expr::Member(expression) => {
            if let Some(diagnostic) = unsupported_payload_enum_value_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(diagnostic) = unsupported_field_member_value_diagnostic(
                sources,
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                diagnostics.push(diagnostic);
            }
            if let Some(root_type) = otherwise_aggregate_member_root_type(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) {
                let otherwise = aggregate_member_root_otherwise(&expression.object)
                    .expect("aggregate otherwise member helper checked expression shape");
                collect_otherwise_aggregate_value_expression_diagnostics(
                    otherwise,
                    &root_type,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            } else {
                collect_expression_diagnostics(
                    &expression.object,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::Index(expression) => {
            if let Some(diagnostic) = unsupported_slice_index_diagnostic(
                sources,
                expression,
                resolved,
                typecheck_facts,
                generic_substitutions,
                resolved_sources,
                nocter_home,
            ) {
                diagnostics.push(diagnostic);
            }
            collect_expression_diagnostics(
                &expression.object,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_expression_diagnostics(
                &expression.index,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::Group(expression) => collect_expression_diagnostics(
            &expression.expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        ),
        Expr::Otherwise(expression) => {
            if !otherwise_optional_value_call_is_buildable(
                &expression.value,
                resolved,
                typecheck_facts,
                generic_substitutions,
                resolved_sources,
            ) {
                diagnostics.push(unsupported_native_build_diagnostic(
                    sources,
                    expression.span,
                    "`otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions",
                    "use `otherwise` directly as a scalar/view value, aggregate member access root, aggregate argument, aggregate field initializer, binding initializer, assignment value, or return expression until general optional expression lowering is promoted",
                ));
            }
            collect_expression_diagnostics(
                &expression.value,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.fallback,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
        }
        Expr::If(expression) => {
            diagnostics.push(unsupported_native_build_diagnostic(
                sources,
                expression.span,
                "`if` expressions",
                "use an explicit `if` statement with `return` until backend expression lowering is promoted",
            ));
            collect_expression_diagnostics(
                &expression.condition,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::IfIs(expression) => {
            let diagnostic = unsupported_if_is_payload_binding_span(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .map(|span| unsupported_payload_binding_diagnostic(sources, span, "`if is`"))
            .unwrap_or_else(|| {
                unsupported_native_build_diagnostic(
                    sources,
                    expression.span,
                    "`if is` expressions",
                    "use an explicit `if is` statement with `return` until backend expression lowering is promoted",
                )
            });
            diagnostics.push(diagnostic);
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            collect_block_diagnostics(
                &expression.then_block,
                None,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            if let Some(else_block) = &expression.else_block {
                collect_block_diagnostics(
                    else_block,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
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
        Expr::Match(expression) => {
            let diagnostic = unsupported_switch_payload_binding_span(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .map(|span| unsupported_payload_binding_diagnostic(sources, span, "`match`"))
            .unwrap_or_else(|| {
                unsupported_native_build_diagnostic(
                    sources,
                    expression.span,
                    "`match` expressions",
                    "use an explicit `match` statement with `return` until backend expression lowering is promoted",
                )
            });
            diagnostics.push(diagnostic);
            collect_expression_diagnostics(
                &expression.expression,
                sources,
                resolved,
                typecheck_facts,
                generic_substitutions,
                root_source,
                names,
                resolved_sources,
                nocter_home,
                queue,
                diagnostics,
            );
            for arm in &expression.arms {
                collect_block_diagnostics(
                    &arm.body,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
                    generic_substitutions,
                    root_source,
                    names,
                    resolved_sources,
                    nocter_home,
                    queue,
                    diagnostics,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_block_diagnostics(
                    &wildcard_arm.body,
                    None,
                    sources,
                    resolved,
                    typecheck_facts,
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
    }
}

fn binary_uses_storage_only_scalar_value(
    expression: &crate::ast::BinaryExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    [&expression.left, &expression.right]
        .into_iter()
        .filter_map(|operand| typecheck_facts.expression_type_expr(operand.span()))
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .any(|ty| {
            type_expr_has_storage_only_scalar_abi_for_sources(&ty, resolved, resolved_sources)
        })
}

fn conversion_uses_computed_storage_only_scalar_value(
    expression: &crate::ast::TypeConversionExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let target_ty = substitute_type_expr_parameters(&expression.ty, generic_substitutions);
    if !type_expr_has_runtime_integer_abi_for_sources(&target_ty, resolved, resolved_sources) {
        return false;
    }

    if matches!(
        unwrap_group_expr(&expression.expression),
        Expr::Identifier(_)
            | Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::Call(_)
            | Expr::Member(_)
            | Expr::Index(_)
            | Expr::Binary(_)
    ) {
        return false;
    }

    let Some(source_ty) = typecheck_facts.expression_type_expr(expression.expression.span()) else {
        return false;
    };
    let source_ty = substitute_type_expr_parameters(source_ty, generic_substitutions);
    type_expr_has_storage_only_scalar_abi_for_sources(&source_ty, resolved, resolved_sources)
}

fn conversion_stores_computed_value_in_storage_only_scalar(
    expression: &crate::ast::TypeConversionExpr,
    resolved: &ResolveOutput,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let target_ty = substitute_type_expr_parameters(&expression.ty, generic_substitutions);
    type_expr_has_storage_only_scalar_abi_for_sources(&target_ty, resolved, resolved_sources)
        && !expression_is_integer_literal_shape(&expression.expression)
}

fn expression_is_integer_literal_shape(expression: &Expr) -> bool {
    match expression {
        Expr::IntegerLiteral(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            expression_is_integer_literal_shape(&unary.operand)
        }
        Expr::Group(group) => expression_is_integer_literal_shape(&group.expression),
        Expr::TypeConversion(conversion) => {
            expression_is_integer_literal_shape(&conversion.expression)
        }
        _ => false,
    }
}
