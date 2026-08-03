use super::*;

pub(in crate::driver::buildability) fn unsupported_local_binding_type_diagnostic(
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

    if fixed_array_move_binding_is_buildable(
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
        if fixed_array_literal_requires_partial_initialization_tracking(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ) {
            return Some(unsupported_v0_build_diagnostic(
                sources,
                statement.initializer.span(),
                "fixed array literal bindings whose element initialization can exit early",
                "initialize every recursively dropped element without `?`, `catch`, `otherwise`, or value control flow until per-element initialization state is tracked",
            ));
        }
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

    if let Some(ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    {
        let source_resolver = |source| resolved_sources.get(&source).copied();
        if type_expr_is_top_level_optional_with_resolver(&ty, resolved, &source_resolver)
            || type_expr_is_top_level_fallible_with_resolver(&ty, resolved, &source_resolver)
        {
            return Some(unsupported_v0_build_diagnostic(
                sources,
                statement.name_span,
                "stored optional or fallible local values",
                "unwrap the value with `?`, `!`, `catch`, or `otherwise` before binding it until optional and fallible local storage is promoted",
            ));
        }
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        statement.name_span,
        "local bindings with unsupported value types",
        "bind `i32`, `u8`, `usize`, `bool`, `&str`, slice views, payloadless enums, errors, aggregate values, or supported fixed array literals until broader scalar local lowering is promoted",
    ))
}

pub(in crate::driver::buildability) fn local_binding_type_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if let Some(ty) = &statement.ty {
        let ty = substitute_type_expr_parameters(ty, generic_substitutions);
        return type_expr_contains_unresolved_type_parameter(&ty, resolved, resolved_sources)
            || local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources);
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
            type_expr_contains_unresolved_type_parameter(&ty, resolved, resolved_sources)
                || local_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
        })
}

pub(in crate::driver::buildability) fn local_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let shape = outcome_shape_with_resolver(ty, resolved, &source_resolver);
    if !shape.layers.is_empty() && shape.is_supported_callable_shape() {
        return type_expr_is_buildable_scalar_or_view_for_sources(
            &shape.payload,
            resolved,
            resolved_sources,
        ) || type_expr_is_supported_aggregate_value_with_resolver(
            &shape.payload,
            resolved,
            &source_resolver,
        );
    }
    type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_error_parameter_for_sources(ty, resolved, resolved_sources)
        || { type_expr_is_supported_borrow_parameter_with_resolver(ty, resolved, &source_resolver) }
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
}

pub(in crate::driver::buildability) fn aggregate_assignment_target_type_expr(
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
