use super::*;

pub(in crate::driver::buildability) fn collect_if_is_target_move_diagnostics(
    statement: &crate::ast::IfIsStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if if_is_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return;
    }

    collect_control_condition_move_diagnostics(
        &statement.expression,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
}

pub(in crate::driver::buildability) fn collect_switch_target_move_diagnostics(
    statement: &crate::ast::SwitchStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return;
    }

    collect_control_condition_move_diagnostics(
        &statement.expression,
        sources,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        diagnostics,
    );
}

pub(in crate::driver::buildability) fn if_is_statement_exits_function_for_buildability(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(else_block) = &statement.else_block else {
        return false;
    };
    block_exits_function_for_buildability(
        &statement.then_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) && block_exits_function_for_buildability(
        else_block,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn switch_statement_exits_function_for_buildability(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.wildcard_arm.is_none()
        && !switch_statement_covers_all_payloadless_variants(statement, resolved)
        && !switch_statement_covers_all_tag_only_payload_variants(statement, resolved)
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        block_exits_function_for_buildability(
            &arm.body,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    }) && statement.wildcard_arm.as_ref().is_none_or(|wildcard_arm| {
        block_exits_function_for_buildability(
            &wildcard_arm.body,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    })
}
