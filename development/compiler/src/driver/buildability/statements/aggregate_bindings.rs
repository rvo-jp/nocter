use super::*;

pub(in crate::driver::buildability) fn assignment_target_root_is_aggregate_binding_for_buildability(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Member(member) => assignment_target_root_is_aggregate_binding_for_buildability(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn identifier_is_outer_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> bool {
    !local_bindings.contains(&identifier.name)
        && identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

pub(in crate::driver::buildability) fn identifier_is_aggregate_for_buildability(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
        return false;
    };
    let Some(ty) = typecheck_facts.binding_type_expr(symbol.name_span) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    type_expr_is_supported_aggregate_value_for_sources(&ty, resolved, resolved_sources)
}
