use super::*;

pub(in crate::driver::buildability) fn type_expr_is_error_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_error_parameter_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}
pub(in crate::driver::buildability) fn type_expr_is_error_parameter_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "error" => true,
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
            let result = type_expr_is_error_parameter_inner(
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
pub(in crate::driver::buildability) fn type_expr_is_supported_borrow_parameter_with_resolver<
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
    let TypeExpr::Borrow(borrow) = ty else {
        return false;
    };
    if !borrow.is_readwrite
        && type_expr_resolves_to_str_with_resolver(&borrow.inner, fallback_resolved, resolver)
    {
        return true;
    }
    if type_expr_resolves_to_view_with_resolver(&borrow.inner, fallback_resolved, resolver) {
        return true;
    }
    abi_value_from_type_expr_with_resolver(&borrow.inner, fallback_resolved, |source| {
        resolver(source)
    })
    .is_ok_and(|value| {
        value.ty.integer_type().is_some()
            || matches!(
                value.ty,
                AbiType::Bool | AbiType::Pointer | AbiType::Struct(_)
            )
    })
}
