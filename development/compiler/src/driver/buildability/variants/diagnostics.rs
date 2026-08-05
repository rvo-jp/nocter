use super::*;

pub(in crate::driver::buildability) fn unsupported_if_is_payload_binding_span(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ByteSpan> {
    if !payload_enum_pattern_target_expression_shape_is_buildable(
        &statement.expression,
        typecheck_facts,
    ) {
        return None;
    }
    let Some(SwitchPayloadPattern::Binding(binding)) = statement.payload.as_ref() else {
        return None;
    };
    (!payload_binding_is_buildable(
        binding,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ))
    .then_some(binding.span)
}

pub(in crate::driver::buildability) fn unsupported_switch_payload_binding_span(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ByteSpan> {
    if !payload_enum_pattern_target_expression_shape_is_buildable(
        &statement.expression,
        typecheck_facts,
    ) {
        return None;
    }
    statement.arms.iter().find_map(|arm| {
        let Some(SwitchPayloadPattern::Binding(binding)) = arm.payload.as_ref() else {
            return None;
        };
        (!payload_binding_is_buildable(
            binding,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ))
        .then_some(binding.span)
    })
}

pub(in crate::driver::buildability) fn unsupported_payload_enum_value_diagnostic(
    sources: &SourceMap,
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let variant_name_span = typecheck_facts.enum_variant_target(member.member_span)?;
    let owner = resolved
        .symbols
        .symbols()
        .find_map(|symbol| match &symbol.kind {
            SymbolKind::Type(type_symbol)
                if type_symbol.kind == TypeSymbolKind::Enum
                    && type_symbol
                        .variants
                        .iter()
                        .any(|variant| variant.name_span == variant_name_span) =>
            {
                Some(type_symbol)
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })?;

    if owner
        .variants
        .iter()
        .all(|variant| variant.payload.is_empty())
    {
        return None;
    }

    if typecheck_facts
        .expression_type_expr(member.span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_some_and(|ty| {
            type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        })
    {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        member.span,
        "payload enum values",
        "use payloadless enum values, or keep payload enum construction on the `check` path until payload enum storage lowering is promoted",
    ))
}
