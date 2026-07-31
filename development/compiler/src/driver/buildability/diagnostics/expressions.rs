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
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
        Expr::InterpolatedString(expression) => {
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "bare string interpolation",
                "construct `String` explicitly with an allocator and `std/fmt.append_*` calls",
            ));
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
            diagnostics.push(unsupported_v0_build_diagnostic(
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
                diagnostics.push(unsupported_v0_build_diagnostic(
                    sources,
                    expression.catch_block.span,
                    "`catch` blocks outside the v0 runtime subset",
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
        Expr::TypeConversion(expression) => collect_expression_diagnostics(
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
        Expr::Call(expression) => {
            let check_only_std_call = unsupported_check_only_std_call_diagnostic(
                sources,
                expression,
                resolved,
                nocter_home,
            );
            if let Some(diagnostic) = &check_only_std_call {
                diagnostics.push(diagnostic.clone());
            }
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
            if check_only_std_call.is_none()
                && unsupported_std_vec_element_call.is_none()
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
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions",
                "use `otherwise` directly as a scalar/view value, aggregate member access root, aggregate argument, aggregate field initializer, binding initializer, assignment value, or return expression until general optional expression lowering is promoted",
            ));
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
            diagnostics.push(unsupported_v0_build_diagnostic(
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
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`if is` expressions",
                "use an explicit `if is` statement with `return` until backend expression lowering is promoted",
            ));
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
            diagnostics.push(unsupported_v0_build_diagnostic(
                sources,
                expression.span,
                "`match` expressions",
                "use an explicit `match` statement with `return` until backend expression lowering is promoted",
            ));
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
