use super::*;

pub(in crate::driver::buildability) fn switch_statement_is_buildable(
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

pub(in crate::driver::buildability) fn payloadless_switch_statement_is_buildable(
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

pub(in crate::driver::buildability) fn tag_only_payload_enum_switch_statement_is_buildable(
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

pub(in crate::driver::buildability) fn payload_enum_pattern_target_expression_is_buildable(
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

pub(in crate::driver::buildability) fn payload_enum_pattern_target_expression_shape_is_buildable(
    expression: &Expr,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) | Expr::Call(_) => true,
        Expr::Member(member) => typecheck_facts
            .enum_variant_target(member.member_span)
            .is_some(),
        Expr::Propagate(propagation) => {
            matches!(unwrap_group_expr(&propagation.expression), Expr::Call(_))
        }
        Expr::Force(force) => {
            matches!(unwrap_group_expr(&force.expression), Expr::Call(_))
        }
        Expr::Catch(catch) => {
            matches!(unwrap_group_expr(&catch.expression), Expr::Call(_))
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            matches!(unwrap_group_expr(&unary.operand), Expr::Identifier(_))
        }
        _ => false,
    }
}

pub(in crate::driver::buildability) fn switch_target_payloadless_enum_symbol<'a>(
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

pub(in crate::driver::buildability) fn payloadless_enum_symbol_for_type_expr<'a, F>(
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

pub(in crate::driver::buildability) fn payload_enum_symbol_for_type_expr<'a, F>(
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
pub(in crate::driver::buildability) enum EnumPayloadRequirement {
    Payloadless,
    Payload,
}

pub(in crate::driver::buildability) fn enum_symbol_for_type_expr<'a, F>(
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

pub(in crate::driver::buildability) fn enum_symbol_for_type_expr_inner<'a, F>(
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

pub(in crate::driver::buildability) fn enum_symbol_matches_payload_requirement(
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

pub(in crate::driver::buildability) fn switch_statement_covers_all_payloadless_variants(
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

pub(in crate::driver::buildability) fn switch_statement_covers_all_tag_only_payload_variants(
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
