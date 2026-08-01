use super::*;

pub(in crate::driver::buildability) fn assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(target) {
        Expr::Identifier(identifier) => Some(local_identifier_type_expr_with_substitutions(
            identifier,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )?),
        Expr::Member(member) => {
            let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
            Some(substitute_type_expr_parameters(&ty, generic_substitutions))
        }
        _ => None,
    }
}
