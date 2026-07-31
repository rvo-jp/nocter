use super::*;

pub(in crate::driver::buildability) fn void_effect_block_is_buildable(
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

pub(in crate::driver::buildability) fn void_effect_expression_is_buildable(
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

pub(in crate::driver::buildability) fn tag_only_payload_pattern_is_buildable(
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

pub(in crate::driver::buildability) fn tag_only_payload_pattern_covers_variant(
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

pub(in crate::driver::buildability) fn collect_terminal_control_condition_move_diagnostics(
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

pub(in crate::driver::buildability) fn condition_explicit_moves_are_single_evaluation_call_for_buildability(
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
