use super::*;

pub(in crate::driver::buildability) fn value_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
) -> bool {
    expression.else_block.is_some()
        && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

pub(in crate::driver::buildability) fn value_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_if_is_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

pub(in crate::driver::buildability) fn value_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_match_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && expression
        .arms
        .iter()
        .all(|arm| value_block_is_buildable(&arm.body))
        && expression
            .wildcard_arm
            .as_ref()
            .is_none_or(|arm| value_block_is_buildable(&arm.body))
}

pub(in crate::driver::buildability) fn value_block_is_buildable(block: &Block) -> bool {
    block.result.is_some()
        && block
            .statements
            .iter()
            .all(value_block_leading_statement_is_buildable)
}

pub(in crate::driver::buildability) fn value_block_leading_statement_is_buildable(
    statement: &Stmt,
) -> bool {
    matches!(
        statement,
        Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Expression(_)
    )
}

pub(in crate::driver::buildability) fn void_effect_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        )
    })
}

pub(in crate::driver::buildability) fn void_effect_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if_is_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        )
    })
}

pub(in crate::driver::buildability) fn void_effect_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_switch_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) && expression.arms.iter().all(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        )
    }) && expression.wildcard_arm.as_ref().is_none_or(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        )
    })
}

pub(in crate::driver::buildability) fn terminal_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
) -> bool {
    expression.else_block.is_some()
}

pub(in crate::driver::buildability) fn terminal_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    expression.else_block.is_some()
        && if_is_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        )
}

pub(in crate::driver::buildability) fn terminal_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let expression_is_exhaustive = expression.wildcard_arm.is_some()
        || switch_statement_covers_all_payloadless_variants(expression, resolved)
        || switch_statement_covers_all_tag_only_payload_variants(expression, resolved);

    expression_is_exhaustive
        && (payloadless_switch_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ) || tag_only_payload_enum_switch_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ))
}
