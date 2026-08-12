use super::*;

pub(in crate::driver::buildability) fn unsupported_field_member_value_diagnostic(
    sources: &SourceMap,
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if typed_hir
        .field_scalar_view_kind(expression.member_span)
        .is_some()
    {
        return None;
    }

    let field_ty = field_type_expr_for_member(expression, resolved, typed_hir)?;
    let field_ty = substitute_type_expr_parameters(&field_ty, generic_substitutions);
    match member_field_value_type_is_buildable(&field_ty, resolved, resolved_sources)? {
        true => None,
        false => Some(unsupported_native_build_diagnostic(
            sources,
            expression.member_span,
            "field member values outside supported scalar/view or aggregate types",
            "expose a value with a concrete native ABI representation",
        )),
    }
}

pub(in crate::driver::buildability) fn field_type_expr_for_member(
    expression: &MemberExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<TypeExpr> {
    field_type_expr_for_span(expression.member_span, resolved, typed_hir)
}

pub(in crate::driver::buildability) fn field_type_expr_for_span(
    field_span: ByteSpan,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<TypeExpr> {
    if let Some(ty) = typed_hir.field_type_expr(field_span) {
        return Some(ty.clone());
    }
    let target = typed_hir.field_target(field_span)?;
    resolved
        .field_signature(target)
        .map(|field| field.ty.clone())
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
    let shape = outcome_shape_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    });
    if !shape.layers.is_empty() && shape.is_supported_callable_shape() {
        return Some(true);
    }
    Some(false)
}

pub(in crate::driver::buildability) fn unsupported_slice_index_diagnostic(
    sources: &SourceMap,
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
    _nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    if let Some(is_buildable) = fixed_array_index_expression_is_buildable(
        expression,
        resolved,
        typed_hir,
        generic_substitutions,
        resolved_sources,
    ) {
        if is_buildable {
            return None;
        }
        return Some(unsupported_native_build_diagnostic(
            sources,
            expression.span,
            "fixed array indexing outside scalar/view element local or aggregate-field reads",
            "index a local or aggregate-field fixed array with builtin integer, `bool`, or `&str` elements",
        ));
    }

    if slice_index_expression_is_buildable(
        expression,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    )? {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        expression.span,
        "slice indexing outside scalar, `&str`, and copy aggregate elements",
        "use a builtin integer, `bool`, `&str`, or a non-empty `copy struct` slice element",
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
            | TypecheckSliceElementKind::Integer(_)
            | TypecheckSliceElementKind::Bool
            | TypecheckSliceElementKind::Str
    )
}
