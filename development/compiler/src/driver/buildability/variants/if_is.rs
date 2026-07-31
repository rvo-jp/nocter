use super::*;

pub(in crate::driver::buildability) fn payloadless_if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
) -> bool {
    if statement.payload.is_some() {
        return false;
    }

    let Some(symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        return false;
    };
    if symbol.kind != TypeSymbolKind::Enum
        || symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    let Some(index) = symbol
        .variants
        .iter()
        .position(|variant| variant.name == statement.variant_name)
    else {
        return false;
    };
    u8::try_from(index).is_ok()
}

pub(in crate::driver::buildability) fn if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_if_is_statement_is_buildable(statement, resolved)
        || tag_only_payload_enum_if_is_statement_is_buildable(
            statement,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

pub(in crate::driver::buildability) fn tag_only_payload_enum_if_is_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        return false;
    };
    if symbol.kind != TypeSymbolKind::Enum
        || symbol.variants.len() > 256
        || symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    let Some(variant) = symbol
        .variants
        .iter()
        .find(|variant| variant.name == statement.variant_name)
    else {
        return false;
    };
    if !tag_only_if_is_payload_pattern_statement_is_buildable(
        statement,
        variant.payload.len(),
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return false;
    }
    payload_enum_pattern_target_expression_is_buildable(
        &statement.expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(in crate::driver::buildability) fn tag_only_if_is_payload_pattern_statement_is_buildable(
    statement: &crate::ast::IfIsStmt,
    payload_len: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match (statement.payload.as_ref(), payload_len) {
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

pub(in crate::driver::buildability) fn payload_binding_is_buildable(
    binding: &crate::ast::SwitchPayloadBinding,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.binding_type_expr(binding.span) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    match typecheck_facts.payload_binding_mode(binding.span) {
        Some(TypecheckPayloadBindingMode::Move) => {
            payload_move_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
        }
        Some(TypecheckPayloadBindingMode::Copy) | None => {
            payload_if_is_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
        }
    }
}

pub(in crate::driver::buildability) fn payload_move_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, source_resolver)
    else {
        return false;
    };
    abi_value_is_supported_aggregate_value(&value)
        && type_expr_has_direct_drop_with_resolver(
            ty,
            fallback_resolved,
            &source_resolver,
            &mut HashSet::new(),
        )
}

pub(in crate::driver::buildability) fn payload_if_is_binding_type_expr_is_buildable(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, source_resolver)
    else {
        return false;
    };
    matches!(
        value.ty,
        AbiType::I32
            | AbiType::U8
            | AbiType::Usize
            | AbiType::Bool
            | AbiType::StrView
            | AbiType::SliceView
    ) || payload_binding_type_expr_is_supported_copy_aggregate(
        ty,
        &value,
        fallback_resolved,
        resolved_sources,
    )
}

pub(in crate::driver::buildability) fn payload_binding_type_expr_is_supported_copy_aggregate(
    ty: &TypeExpr,
    value: &AbiValue,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    if !abi_value_is_supported_aggregate_value(value) {
        return false;
    }
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_runtime_copy_value_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
        &mut HashSet::new(),
    )
}
