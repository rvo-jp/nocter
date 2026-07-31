use super::*;

pub(in crate::driver::buildability) fn unsupported_outer_aggregate_move_binding_span(
    statement: &BindingStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if direct_outer_aggregate_move_for_buildability(
        &statement.initializer,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }
    Some(span)
}

pub(in crate::driver::buildability) fn unsupported_outer_aggregate_move_assignment_span(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    let span = expression_explicit_outer_aggregate_move_span(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )?;
    if assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
        statement,
        statements,
        index,
        result,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) {
        return None;
    }
    Some(span)
}

pub(in crate::driver::buildability) fn assignment_outer_aggregate_move_before_function_exit_allowed_for_buildability(
    statement: &AssignmentStmt,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    direct_outer_aggregate_move_for_buildability(
        &statement.value,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    ) && assignment_target_root_is_aggregate_binding_for_buildability(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && statement_suffix_exits_function_for_buildability(
        statements,
        index,
        result,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn direct_outer_aggregate_move_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    let Expr::Unary(unary) = unwrap_group_expr(expression) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) else {
        return false;
    };
    identifier_is_outer_aggregate_for_buildability(
        identifier,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        local_bindings,
    )
}
