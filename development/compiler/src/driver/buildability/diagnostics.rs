use super::*;

pub(super) fn unsupported_local_binding_type_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if fixed_array_literal_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_copy_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_call_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_member_binding_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    let fixed_array_binding_type = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    );
    if fixed_array_binding_type.is_some() {
        return Some(match unwrap_group_expr(&statement.initializer) {
            Expr::ArrayLiteral(_) => unsupported_v0_build_diagnostic(
                sources,
                statement.initializer.span(),
                "fixed array local bindings outside supported literal values",
                "match the fixed array length and use `i32`, `u8`, `usize`, `bool`, or `&str` elements until broader fixed array element storage is promoted",
            ),
            _ => unsupported_v0_build_diagnostic(
                sources,
                statement.name_span,
                "fixed array local bindings outside supported initialization",
                "initialize fixed array locals directly from a supported array literal, copy another supported fixed array local or aggregate field, or bind a matching fixed array call result until broader fixed array move lowering is promoted",
            ),
        });
    }

    if local_binding_type_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        statement.name_span,
        "local bindings with unsupported value types",
        "bind `i32`, `u8`, `usize`, `bool`, `&str`, slice views, payloadless enums, errors, aggregate values, or supported fixed array literals until broader scalar local lowering is promoted",
    ))
}

pub(super) fn local_binding_type_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if let Some(ty) = &statement.ty {
        let ty = substitute_type_expr_parameters(ty, generic_substitutions);
        return local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
            || !type_expr_is_known_unsupported_scalar_value_for_sources(
                &ty,
                resolved,
                resolved_sources,
            );
    }

    if typecheck_facts
        .binding_scalar_view_kind(statement.name_span)
        .is_some()
    {
        return true;
    }

    typecheck_facts
        .binding_type_expr(statement.name_span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_none_or(|ty| {
            local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
                || !type_expr_is_known_unsupported_scalar_value_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
        })
}

pub(super) fn unsupported_scalar_type_label(label: &str) -> bool {
    matches!(
        label,
        "i8" | "i16" | "i64" | "isize" | "u16" | "u32" | "u64"
    )
}

pub(super) fn local_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_error_parameter_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
}

pub(super) fn aggregate_assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let ty = assignment_target_type_expr(target, resolved, typecheck_facts, generic_substitutions)?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(super) fn void_effect_block_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match block.result.as_deref() {
        Some(result) => void_effect_expression_is_buildable(
            result,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        None => true,
    }
}

pub(super) fn void_effect_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(expression) => void_effect_if_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::IfIs(expression) => void_effect_if_is_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Match(expression) => void_effect_match_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => expression_statement_is_supported(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
    }
}

pub(super) fn tag_only_payload_pattern_is_buildable(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match (payload, payload_len) {
        (None, 0) | (Some(SwitchPayloadPattern::Discard(_)), 1) => true,
        (Some(SwitchPayloadPattern::Binding(binding)), 1) => payload_binding_is_buildable(
            binding,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(super) fn tag_only_payload_pattern_covers_variant(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

pub(super) fn collect_terminal_control_condition_move_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(span) = expression_explicit_aggregate_move_span(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return;
    };
    if condition_explicit_moves_are_single_evaluation_call_for_buildability(expression) {
        return;
    }

    diagnostics.push(unsupported_v0_build_diagnostic(
        sources,
        span,
        "explicit aggregate moves in control-flow conditions",
        "use a single call expression for terminal branch conditions that move aggregate values, or move aggregate values after branch selection until broader condition move lowering is promoted",
    ));
}

pub(super) fn condition_explicit_moves_are_single_evaluation_call_for_buildability(
    expression: &Expr,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&unary.operand)
        }
        Expr::Propagate(propagation) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(
                &propagation.expression,
            )
        }
        Expr::Force(force) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&force.expression)
        }
        Expr::Catch(catch) => {
            condition_explicit_moves_are_single_evaluation_call_for_buildability(&catch.expression)
        }
        _ => false,
    }
}

pub(super) fn unsupported_expression_statement_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if expression_statement_is_supported(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span(),
        "value-producing expression statements",
        "call a void, never, or discardable scalar/view/aggregate function, handle a discardable scalar/view/aggregate fallible call with `?`, `!`, or `catch`, or bind/return the value explicitly",
    ))
}

pub(super) fn otherwise_optional_value_call_is_buildable(
    value: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::Call(call) = unwrap_group_expr(value) else {
        return false;
    };
    let Some(return_type) = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_top_level_optional_with_resolver(&return_type, resolved, &source_resolver)
}

pub(super) fn expression_is_never_runtime_shape_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => matches!(
            call_return_shape(call, resolved, typecheck_facts, generic_substitutions),
            Some(ReturnShape::Never)
        ),
        _ => false,
    }
}

pub(super) fn aggregate_literal_statement_is_supported(
    literal: &crate::ast::StructLiteralExpr,
    resolved: &ResolveOutput,
) -> bool {
    abi_value_from_type_expr(&literal.ty, resolved)
        .map(|value| matches!(value.ty, AbiType::Struct(_)))
        .unwrap_or(false)
}

pub(super) fn unsupported_index_assignment_target_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    let Expr::Index(index) = unwrap_group_expr(&statement.target) else {
        return None;
    };
    if let Some(is_buildable) = fixed_array_index_assignment_target_is_buildable(
        index,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_v0_build_diagnostic(
            sources,
            index.span,
            "fixed array index assignment targets outside scalar/view element locals or aggregate fields",
            "assign through an index into a local or aggregate-field `[i32; N]`, `[u8; N]`, `[usize; N]`, `[bool; N]`, or `[&str; N]` until broader fixed array mutation is promoted",
        ));
    }
    if matches!(
        slice_index_assignment_target_is_buildable(
            &index.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Some(true) | None
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        index.object.span(),
        "index assignment targets outside supported slice values",
        "assign through a slice binding, supported slice-returning call result, or slice aggregate field until broader index assignment lowering is promoted",
    ))
}

pub(super) fn collect_expression_diagnostics(
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

pub(super) fn unsupported_field_member_value_diagnostic(
    sources: &SourceMap,
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if typecheck_facts
        .field_scalar_view_kind(expression.member_span)
        .is_some()
    {
        return None;
    }

    let field_ty = field_type_expr_for_member(expression, resolved, typecheck_facts)?;
    let field_ty = substitute_type_expr_parameters(&field_ty, generic_substitutions);
    match member_field_value_type_is_buildable(&field_ty, resolved, resolved_sources)? {
        true => None,
        false => Some(unsupported_v0_build_diagnostic(
            sources,
            expression.member_span,
            "field member values outside supported scalar/view or aggregate types",
            "keep `u16`, `u32`, and other storage-only fields encapsulated in aggregates, or expose an `i32`, `usize`, or `u8` value until broader scalar field lowering is promoted",
        )),
    }
}

pub(super) fn field_type_expr_for_member(
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    field_type_expr_for_span(expression.member_span, resolved, typecheck_facts)
}

pub(super) fn field_type_expr_for_span(
    field_span: ByteSpan,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    if let Some(ty) = typecheck_facts.field_type_expr(field_span) {
        return Some(ty.clone());
    }
    let target_span = typecheck_facts.field_target(field_span)?;
    resolved.symbols.symbols().find_map(|symbol| {
        let SymbolKind::Type(type_symbol) = &symbol.kind else {
            return None;
        };
        type_symbol
            .fields
            .iter()
            .find(|field| field.name_span == target_span)
            .map(|field| field.ty.clone())
    })
}

pub(super) fn member_field_value_type_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    if type_expr_contains_unresolved_type_parameter(ty, resolved, resolved_sources) {
        return None;
    }
    if type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
    {
        return Some(true);
    }
    Some(false)
}

pub(super) fn unsupported_slice_index_diagnostic(
    sources: &SourceMap,
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    // `std/vec` generic bodies keep parameter element facts as `Other`; user
    // call sites are preflighted before those bodies are lowered.
    if source_is_std_vec(sources, expression.span.source, nocter_home) {
        return None;
    }

    if let Some(is_buildable) = fixed_array_index_expression_is_buildable(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_v0_build_diagnostic(
            sources,
            expression.span,
            "fixed array indexing outside scalar/view element local or aggregate-field reads",
            "index a local or aggregate-field `[i32; N]`, `[u8; N]`, `[usize; N]`, `[bool; N]`, or `[&str; N]` value until broader fixed array indexing is promoted",
        ));
    }

    if slice_index_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )? {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span,
        "slice indexing outside scalar, `&str`, and copy aggregate elements",
        "use `&[i32]`, `&[u8]`, `&[usize]`, `&[bool]`, `&[&str]`, or a non-empty `copy struct` element until broader slice element lowering is promoted",
    ))
}

pub(super) fn typecheck_slice_element_kind_is_buildable(
    element: TypecheckSliceElementKind,
) -> bool {
    matches!(
        element,
        TypecheckSliceElementKind::I32
            | TypecheckSliceElementKind::U8
            | TypecheckSliceElementKind::Usize
            | TypecheckSliceElementKind::Bool
            | TypecheckSliceElementKind::Str
    )
}

pub(super) fn unsupported_unloaded_imported_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    let SymbolKind::Imported(imported) = &symbol.kind else {
        return None;
    };

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "unloaded imported function calls",
        &format!(
            "load `{}` from the active Nocter home or use a same-file function until imported placeholder lowering is promoted",
            imported.path
        ),
    ))
}

pub(super) fn unsupported_borrow_call_argument_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let argument = call
        .arguments
        .iter()
        .enumerate()
        .find_map(|(index, argument)| {
            let parameter_ty = call_argument_parameter_type(
                call,
                index,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            if !type_expr_resolves_to_borrow_with_resolver(
                &parameter_ty,
                resolved,
                &source_resolver,
            ) {
                return None;
            }
            match unwrap_group_expr(argument) {
                Expr::Borrow(borrow)
                    if borrow.is_readwrite
                        && !readwrite_borrow_argument_source_is_buildable(
                            &borrow.expression,
                            resolved,
                            resolved_sources,
                            typecheck_facts,
                            generic_substitutions,
                        ) =>
                {
                    Some(argument)
                }
                _ => None,
            }
        })?;

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "read-write borrow call arguments from unsupported expressions",
        "borrow a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary borrow lowering is promoted",
    ))
}

pub(super) fn unsupported_method_borrow_receiver_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typecheck_facts.method_call_target(member.member_span)?;
    if !method_call_receiver_is_readwrite_borrow(member.member_span, typecheck_facts) {
        return None;
    }
    if readwrite_borrow_argument_source_is_buildable(
        &member.object,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.object.span(),
        "read-write method borrow receivers from unsupported expressions",
        "call the method on a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary receiver lowering is promoted",
    ))
}

pub(super) fn unsupported_unspecialized_generic_method_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typecheck_facts.generic_method_call_target(member.member_span)?;
    if concrete_method_call_specialization(member, typecheck_facts, generic_substitutions).is_some()
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "generic impl method calls without concrete type arguments",
        "call the method through a receiver whose generic arguments are concrete until generic method bodies can be re-specialized recursively",
    ))
}

pub(super) fn concrete_method_call_specialization(
    member: &crate::ast::MemberExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<MethodCallSpecialization> {
    typecheck_facts
        .method_call_specialization(member.member_span)?
        .with_context_substitutions(generic_substitutions)
}

pub(super) fn unsupported_unspecialized_generic_function_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    typecheck_facts.generic_function_call_target(call.span)?;
    if concrete_function_call_specialization(call, typecheck_facts, generic_substitutions).is_some()
    {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        call.span,
        "generic function calls without concrete type arguments",
        "make every generic parameter concrete through argument types or return context",
    ))
}

pub(super) fn concrete_function_call_specialization(
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<FunctionCallSpecialization> {
    typecheck_facts
        .function_call_specialization(call.span)?
        .with_context_substitutions(generic_substitutions)
}

pub(super) fn unwrap_group_expr(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group_expr(&group.expression),
        _ => expression,
    }
}

pub(super) fn unsupported_v0_build_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    construct: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0435",
        format!("Nocter v0 build cannot lower {construct} yet"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}

pub(super) fn call_target_for_source(
    source: SourceId,
    root_source: SourceId,
    name: String,
) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

pub(super) fn span_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    outer.source == inner.source && outer.start <= inner.start && inner.end <= outer.end
}

pub(super) fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

pub(super) fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", type_expr_display_lossy(self_ty))
}

pub(super) fn nested_fallible_return_issue(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    nested_fallible_return_type_issue(
        &return_type,
        function.return_type.span(),
        resolved,
        resolved_sources,
    )
}

pub(super) fn nested_fallible_return_type_issue(
    return_type: &TypeExpr,
    diagnostic_span: ByteSpan,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<BuildabilityIssue> {
    if type_expr_fallible_depth(return_type, resolved, resolved_sources) <= 1 {
        return None;
    }

    Some(BuildabilityIssue {
        span: diagnostic_span,
        construct: "nested fallible or optional return types",
        help: "flatten the return boundary to a single optional or fallible layer until nested fallible lowering is promoted",
    })
}

pub(super) fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

pub(super) fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
}
