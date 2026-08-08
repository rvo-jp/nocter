use super::*;

pub(in crate::driver::buildability) fn type_expr_resolves_to_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_view_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}
pub(in crate::driver::buildability) fn type_expr_resolves_to_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(_) => true,
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_resolves_to_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}
pub(in crate::driver::buildability) fn type_expr_resolves_to_supported_slice_view_with_resolver<
    'a,
    F,
>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_supported_slice_view_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}
pub(in crate::driver::buildability) fn type_expr_resolves_to_supported_slice_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(view) => Some(type_expr_is_supported_slice_index_element_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        )),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_resolves_to_supported_slice_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}
pub(in crate::driver::buildability) fn type_expr_resolved_view_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolved_view_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}
pub(in crate::driver::buildability) fn type_expr_resolved_view_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::View(view) => Some(type_expr_slice_element_kind_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        )),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_resolved_view_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}
pub(in crate::driver::buildability) fn type_expr_is_supported_slice_index_element_with_resolver<
    'a,
    F,
>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_slice_element_kind_with_resolver(ty, fallback_resolved, resolver)
        != TypecheckSliceElementKind::Other
        || type_expr_is_supported_copy_aggregate_vec_element_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        )
}
pub(in crate::driver::buildability) fn type_expr_slice_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_slice_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}
pub(in crate::driver::buildability) fn type_expr_slice_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let TypeExpr::Reference(reference) = ty
        && let Some(kind) = IntegerType::from_name(&reference.name)
    {
        return match kind {
            IntegerType::I32 => TypecheckSliceElementKind::I32,
            IntegerType::U8 => TypecheckSliceElementKind::U8,
            IntegerType::Usize => TypecheckSliceElementKind::Usize,
            kind => TypecheckSliceElementKind::Integer(kind),
        };
    }
    match ty {
        TypeExpr::Reference(reference) if reference.name == "bool" => {
            TypecheckSliceElementKind::Bool
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            TypecheckSliceElementKind::Str
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return TypecheckSliceElementKind::Other;
            };
            if symbol.kind != TypeSymbolKind::Alias {
                return TypecheckSliceElementKind::Other;
            }
            let Some(target) = &symbol.alias_target else {
                return TypecheckSliceElementKind::Other;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return TypecheckSliceElementKind::Other;
            }
            let kind = type_expr_slice_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            kind
        }
        _ => TypecheckSliceElementKind::Other,
    }
}
