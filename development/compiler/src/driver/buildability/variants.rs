use super::*;

pub(super) fn payload_enum_variant_payloads_are_supported<'a, F>(
    payloads: &[crate::resolve::ParameterSignature],
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match payloads {
        [] => true,
        [payload] => {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }
        payloads => payloads.iter().all(|payload| {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }),
    }
}

pub(super) fn payload_enum_payload_type_is_supported<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    allow_active_drop: bool,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok()
        && (type_expr_is_runtime_copy_value_with_resolver(
            ty,
            fallback_resolved,
            resolver,
            &mut HashSet::new(),
        ) || (allow_active_drop
            && type_expr_has_direct_drop_with_resolver(
                ty,
                fallback_resolved,
                resolver,
                &mut HashSet::new(),
            )))
}

pub(super) fn value_if_expression_is_buildable(expression: &crate::ast::IfStmt) -> bool {
    expression.else_block.is_some()
        && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

pub(super) fn value_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_if_is_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && value_block_is_buildable(&expression.then_block)
        && expression
            .else_block
            .as_ref()
            .is_some_and(value_block_is_buildable)
}

pub(super) fn value_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    terminal_match_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
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

pub(super) fn value_block_is_buildable(block: &Block) -> bool {
    block.result.is_some()
        && block
            .statements
            .iter()
            .all(value_block_leading_statement_is_buildable)
}

pub(super) fn value_block_leading_statement_is_buildable(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Expression(_)
    )
}

pub(super) fn void_effect_if_expression_is_buildable(
    expression: &crate::ast::IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(super) fn void_effect_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if_is_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && void_effect_block_is_buildable(
        &expression.then_block,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.else_block.as_ref().is_none_or(|block| {
        void_effect_block_is_buildable(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(super) fn void_effect_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_switch_statement_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) && expression.arms.iter().all(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    }) && expression.wildcard_arm.as_ref().is_none_or(|arm| {
        void_effect_block_is_buildable(
            &arm.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(super) fn terminal_if_expression_is_buildable(expression: &crate::ast::IfStmt) -> bool {
    expression.else_block.is_some()
}

pub(super) fn terminal_if_is_expression_is_buildable(
    expression: &crate::ast::IfIsStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    expression.else_block.is_some()
        && if_is_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
}

pub(super) fn terminal_match_expression_is_buildable(
    expression: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
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
            typecheck_facts,
            generic_substitutions,
        ) || tag_only_payload_enum_switch_statement_is_buildable(
            expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ))
}

pub(super) fn payloadless_if_is_statement_is_buildable(
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

pub(super) fn if_is_statement_is_buildable(
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

pub(super) fn tag_only_payload_enum_if_is_statement_is_buildable(
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

pub(super) fn tag_only_if_is_payload_pattern_statement_is_buildable(
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

pub(super) fn payload_binding_is_buildable(
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
    payload_if_is_binding_type_expr_is_buildable(&ty, resolved, resolved_sources)
}

pub(super) fn payload_if_is_binding_type_expr_is_buildable(
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

pub(super) fn payload_binding_type_expr_is_supported_copy_aggregate(
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

pub(super) fn switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    payloadless_switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || tag_only_payload_enum_switch_statement_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn payloadless_switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return statement.wildcard_arm.is_some()
            && switch_target_payloadless_enum_symbol(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
            .is_some();
    };
    if statement.arms.iter().any(|arm| arm.payload.is_some()) {
        return false;
    }

    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        resolved
            .type_symbol_by_name(&arm.enum_name)
            .is_some_and(|symbol| symbol.canonical_name == target_symbol.canonical_name)
            && target_symbol
                .variants
                .iter()
                .any(|variant| variant.name == arm.variant_name)
    })
}

pub(super) fn tag_only_payload_enum_switch_statement_is_buildable(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.expression_type_expr(statement.expression.span()) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    if !type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources) {
        return false;
    }
    if !payload_enum_pattern_target_expression_shape_is_buildable(
        &statement.expression,
        typecheck_facts,
    ) {
        return false;
    }

    let Some(first_arm) = statement.arms.first() else {
        let source_resolver = |source| resolved_sources.get(&source).copied();
        return statement.wildcard_arm.is_some()
            && payload_enum_symbol_for_type_expr(&ty, resolved, &source_resolver).is_some();
    };

    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    statement.arms.iter().all(|arm| {
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let Some(variant) = target_symbol
            .variants
            .iter()
            .find(|variant| variant.name == arm.variant_name)
        else {
            return false;
        };
        tag_only_payload_pattern_is_buildable(
            arm.payload.as_ref(),
            variant.payload.len(),
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
    })
}

pub(super) fn payload_enum_pattern_target_expression_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(ty) = typecheck_facts.expression_type_expr(expression.span()) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        && payload_enum_pattern_target_expression_shape_is_buildable(expression, typecheck_facts)
}

pub(super) fn payload_enum_pattern_target_expression_shape_is_buildable(
    expression: &Expr,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) | Expr::Call(_) => true,
        Expr::Member(member) => typecheck_facts
            .enum_variant_target(member.member_span)
            .is_some(),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            matches!(unwrap_group_expr(&unary.operand), Expr::Identifier(_))
        }
        _ => false,
    }
}

pub(super) fn switch_target_payloadless_enum_symbol<'a>(
    statement: &crate::ast::SwitchStmt,
    resolved: &'a ResolveOutput,
    resolved_sources: &ResolvedSources<'a>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<&'a TypeSymbol> {
    let ty = typecheck_facts.expression_type_expr(statement.expression.span())?;
    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    payloadless_enum_symbol_for_type_expr(&ty, resolved, &source_resolver)
}

pub(super) fn payloadless_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payloadless,
    )
}

pub(super) fn payload_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payload,
    )
}

#[derive(Clone, Copy)]
pub(super) enum EnumPayloadRequirement {
    Payloadless,
    Payload,
}

pub(super) fn enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr_inner(
        ty,
        fallback_resolved,
        resolver,
        payload_requirement,
        &mut HashSet::new(),
    )
}

pub(super) fn enum_symbol_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
    resolving_names: &mut HashSet<String>,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            (generic.name.as_str(), substitutions)
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => return None,
    };
    let symbol = type_symbol_by_reference_name(resolved, type_name)?;
    if symbol.kind == TypeSymbolKind::Alias {
        let target = symbol.alias_target.as_ref()?;
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let result = enum_symbol_for_type_expr_inner(
            &target,
            fallback_resolved,
            resolver,
            payload_requirement,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return result;
    }

    enum_symbol_matches_payload_requirement(symbol, payload_requirement).then_some(symbol)
}

pub(super) fn enum_symbol_matches_payload_requirement(
    symbol: &TypeSymbol,
    payload_requirement: EnumPayloadRequirement,
) -> bool {
    if symbol.kind != TypeSymbolKind::Enum || symbol.variants.len() > 256 {
        return false;
    }

    match payload_requirement {
        EnumPayloadRequirement::Payloadless => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        EnumPayloadRequirement::Payload => symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty()),
    }
}

pub(super) fn switch_statement_covers_all_payloadless_variants(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return false;
    };
    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return false;
    }

    let covered = statement
        .arms
        .iter()
        .filter_map(|arm| {
            let pattern_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
            if pattern_symbol.kind != TypeSymbolKind::Enum
                || pattern_symbol.canonical_name != target_symbol.canonical_name
                || arm.payload.is_some()
            {
                return None;
            }

            target_symbol
                .variants
                .iter()
                .find(|variant| variant.name == arm.variant_name)
                .map(|variant| variant.name.as_str())
        })
        .collect::<HashSet<_>>();
    target_symbol
        .variants
        .iter()
        .all(|variant| covered.contains(variant.name.as_str()))
}

pub(super) fn switch_statement_covers_all_tag_only_payload_variants(
    statement: &crate::ast::SwitchStmt,
    resolved: &ResolveOutput,
) -> bool {
    let Some(first_arm) = statement.arms.first() else {
        return false;
    };
    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return false;
    };
    if target_symbol.kind != TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    let covered = statement
        .arms
        .iter()
        .filter_map(|arm| {
            let arm_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
            if arm_symbol.kind != TypeSymbolKind::Enum
                || arm_symbol.canonical_name != target_symbol.canonical_name
            {
                return None;
            }
            let variant = target_symbol
                .variants
                .iter()
                .find(|variant| variant.name == arm.variant_name)?;
            tag_only_payload_pattern_covers_variant(arm.payload.as_ref(), variant.payload.len())
                .then_some(variant.name.as_str())
        })
        .collect::<HashSet<_>>();

    target_symbol
        .variants
        .iter()
        .all(|variant| covered.contains(variant.name.as_str()))
}

pub(super) fn collect_if_is_target_move_diagnostics(
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

pub(super) fn collect_switch_target_move_diagnostics(
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

pub(super) fn if_is_statement_exits_function_for_buildability(
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

pub(super) fn switch_statement_exits_function_for_buildability(
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

pub(super) fn unsupported_payload_enum_value_diagnostic(
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

    Some(unsupported_v0_build_diagnostic(
        sources,
        member.span,
        "payload enum values",
        "use payloadless enum values, or keep payload enum construction on the `check` path until payload enum storage lowering is promoted",
    ))
}

pub(super) fn payload_enum_constructor_call_is_supported(
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Member(member) = call.callee.as_ref() else {
        return false;
    };
    if typecheck_facts
        .enum_variant_target(member.member_span)
        .is_none()
    {
        return false;
    }
    typecheck_facts
        .expression_type_expr(call.span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_some_and(|ty| {
            type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        })
}
