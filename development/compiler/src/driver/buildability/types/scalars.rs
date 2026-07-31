use super::*;

pub(in crate::driver::buildability) fn type_expr_has_str_view_abi_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver)
        .is_ok_and(|value| matches!(value.ty, AbiType::StrView))
}
pub(in crate::driver::buildability) fn type_expr_is_known_unsupported_scalar_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_known_unsupported_scalar_value_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}
pub(in crate::driver::buildability) fn type_expr_is_known_unsupported_scalar_value_with_resolver<
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
    type_expr_is_known_unsupported_scalar_value_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}
pub(in crate::driver::buildability) fn type_expr_is_known_unsupported_scalar_value_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if unsupported_scalar_type_label(&reference.name) => true,
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
            let result = type_expr_is_known_unsupported_scalar_value_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok_and(
            |value| {
                matches!(
                    value.ty,
                    AbiType::I8
                        | AbiType::I16
                        | AbiType::I64
                        | AbiType::Isize
                        | AbiType::U16
                        | AbiType::U32
                        | AbiType::U64
                )
            },
        ),
    }
}
pub(in crate::driver::buildability) fn type_expr_is_buildable_scalar_or_view_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, &source_resolver)
}
pub(in crate::driver::buildability) fn type_expr_is_buildable_scalar_or_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_buildable_scalar_or_view_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}
pub(in crate::driver::buildability) fn type_expr_is_buildable_scalar_or_view_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference)
            if matches!(reference.name.as_str(), "i32" | "u8" | "usize" | "bool") =>
        {
            true
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            true
        }
        TypeExpr::Borrow(borrow)
            if type_expr_resolves_to_view_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            ) =>
        {
            true
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return type_expr_has_buildable_scalar_abi_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return type_expr_has_buildable_scalar_abi_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_buildable_scalar_or_view_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => type_expr_has_buildable_scalar_abi_with_resolver(ty, fallback_resolved, resolver),
    }
}
pub(in crate::driver::buildability) fn type_expr_has_buildable_scalar_abi_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok_and(|value| {
        matches!(
            value.ty,
            AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::Pointer
        )
    })
}
