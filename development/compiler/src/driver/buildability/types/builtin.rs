use super::*;

pub(in crate::driver::buildability) fn type_expr_resolves_to_str_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_builtin_reference_inner(
        ty,
        fallback_resolved,
        resolver,
        "str",
        &mut HashSet::new(),
    )
}
pub(in crate::driver::buildability) fn type_expr_resolves_to_builtin_reference_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    expected: &str,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == expected => true,
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
            let result = type_expr_resolves_to_builtin_reference_inner(
                target,
                fallback_resolved,
                resolver,
                expected,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}
