use super::*;

pub(in crate::driver::buildability) fn void_effect_block_is_buildable(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match block.result.as_deref() {
        Some(result) => void_effect_expression_is_buildable(
            result,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
        None => true,
    }
}

pub(in crate::driver::buildability) fn void_effect_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::If(expression) => void_effect_if_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
        Expr::IfIs(expression) => void_effect_if_is_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
        Expr::Match(expression) => void_effect_match_expression_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
        _ => expression_statement_is_supported(
            expression,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions,
        ),
    }
}

pub(in crate::driver::buildability) fn tag_only_payload_pattern_is_buildable(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match (payload, payload_len) {
        (None, 0) | (Some(SwitchPayloadPattern::Discard(_)), 1) => true,
        (Some(SwitchPayloadPattern::Binding(binding)), 1) => payload_binding_is_buildable(
            binding,
            resolved,
            resolved_sources,
            typed_hir,
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
