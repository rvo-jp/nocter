use super::*;

pub(in crate::driver::buildability) fn unsupported_field_member_value_diagnostic(
    sources: &SourceMap,
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if typecheck_facts
        .field_scalar_view_kind(expression.member_span)
        .is_some()
    {
        return None;
    }

    let field_ty = field_type_expr_for_member(expression, resolved, typecheck_facts)?;
    let field_ty = substitute_type_expr_parameters(&field_ty, generic_substitutions);
    match member_field_value_type_is_buildable(&field_ty, resolved, resolved_sources)? {
        true => None,
        false => Some(unsupported_v0_build_diagnostic(
            sources,
            expression.member_span,
            "field member values outside supported scalar/view or aggregate types",
            "keep `u16`, `u32`, and other storage-only fields encapsulated in aggregates, or expose an `i32`, `usize`, or `u8` value until broader scalar field lowering is promoted",
        )),
    }
}

pub(in crate::driver::buildability) fn field_type_expr_for_member(
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    field_type_expr_for_span(expression.member_span, resolved, typecheck_facts)
}

pub(in crate::driver::buildability) fn field_type_expr_for_span(
    field_span: ByteSpan,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Option<TypeExpr> {
    if let Some(ty) = typecheck_facts.field_type_expr(field_span) {
        return Some(ty.clone());
    }
    let target_span = typecheck_facts.field_target(field_span)?;
    resolved.symbols.symbols().find_map(|symbol| {
        let SymbolKind::Type(type_symbol) = &symbol.kind else {
            return None;
        };
        type_symbol
            .fields
            .iter()
            .find(|field| field.name_span == target_span)
            .map(|field| field.ty.clone())
    })
}

pub(in crate::driver::buildability) fn member_field_value_type_is_buildable(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    if type_expr_contains_unresolved_type_parameter(ty, resolved, resolved_sources) {
        return None;
    }
    if type_expr_is_buildable_scalar_or_view_for_sources(ty, resolved, resolved_sources)
        || type_expr_is_supported_aggregate_value_for_sources(ty, resolved, resolved_sources)
    {
        return Some(true);
    }
    Some(false)
}

pub(in crate::driver::buildability) fn unsupported_slice_index_diagnostic(
    sources: &SourceMap,
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    // `std/vec` generic bodies keep parameter element facts as `Other`; user
    // call sites are preflighted before those bodies are lowered.
    if source_is_std_vec(sources, expression.span.source, nocter_home) {
        return None;
    }

    if let Some(is_buildable) = fixed_array_index_expression_is_buildable(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_v0_build_diagnostic(
            sources,
            expression.span,
            "fixed array indexing outside scalar/view element local or aggregate-field reads",
            "index a local or aggregate-field `[i32; N]`, `[u8; N]`, `[usize; N]`, `[bool; N]`, or `[&str; N]` value until broader fixed array indexing is promoted",
        ));
    }

    if slice_index_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )? {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        expression.span,
        "slice indexing outside scalar, `&str`, and copy aggregate elements",
        "use `&[i32]`, `&[u8]`, `&[usize]`, `&[bool]`, `&[&str]`, or a non-empty `copy struct` element until broader slice element lowering is promoted",
    ))
}

pub(in crate::driver::buildability) fn typecheck_slice_element_kind_is_buildable(
    element: TypecheckSliceElementKind,
) -> bool {
    matches!(
        element,
        TypecheckSliceElementKind::I32
            | TypecheckSliceElementKind::U8
            | TypecheckSliceElementKind::Usize
            | TypecheckSliceElementKind::Bool
            | TypecheckSliceElementKind::Str
    )
}
