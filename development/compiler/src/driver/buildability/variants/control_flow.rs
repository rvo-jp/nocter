use super::*;

pub(in crate::driver::buildability) fn if_is_statement_exits_function_for_buildability(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typed_hir,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typed_hir,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn switch_statement_exits_function_for_buildability(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.wildcard_arm.is_none()
        && !switch_statement_covers_all_payloadless_variants(statement, resolved)
        && !switch_statement_covers_all_tag_only_payload_variants(statement, resolved)
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        block_exits_function_for_buildability(&arm.body, resolved, typed_hir, generic_substitutions)
    }) && statement.wildcard_arm.as_ref().is_none_or(|wildcard_arm| {
        block_exits_function_for_buildability(
            &wildcard_arm.body,
            resolved,
            typed_hir,
            generic_substitutions,
        )
    })
}
