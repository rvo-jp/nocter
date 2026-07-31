use super::*;

pub(in crate::driver::buildability) fn unsupported_expression_statement_diagnostic(
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

pub(in crate::driver::buildability) fn otherwise_optional_value_call_is_buildable(
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

pub(in crate::driver::buildability) fn expression_is_never_runtime_shape_is_buildable(
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

pub(in crate::driver::buildability) fn aggregate_literal_statement_is_supported(
    literal: &crate::ast::StructLiteralExpr,
    resolved: &ResolveOutput,
) -> bool {
    abi_value_from_type_expr(&literal.ty, resolved)
        .map(|value| matches!(value.ty, AbiType::Struct(_)))
        .unwrap_or(false)
}

pub(in crate::driver::buildability) fn unsupported_index_assignment_target_diagnostic(
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
