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
pub(in crate::driver::buildability) fn type_expr_contains_unresolved_type_parameter(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_contains_unresolved_type_parameter_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}
pub(in crate::driver::buildability) fn type_expr_contains_unresolved_type_parameter_with_resolver<
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
    match ty {
        TypeExpr::Callable(callable) => {
            callable.parameters.iter().any(|parameter| {
                type_expr_contains_unresolved_type_parameter_with_resolver(
                    &parameter.ty,
                    fallback_resolved,
                    resolver,
                )
            }) || type_expr_contains_unresolved_type_parameter_with_resolver(
                &callable.return_type,
                fallback_resolved,
                resolver,
            )
        }
        TypeExpr::Closure(closure) => closure.captures.iter().any(|capture| {
            type_expr_contains_unresolved_type_parameter_with_resolver(
                &capture.ty,
                fallback_resolved,
                resolver,
            )
        }),
        TypeExpr::Projection(_) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            crate::typecheck::normalize_associated_type_expr(ty, resolved).is_none_or(|ty| {
                type_expr_contains_unresolved_type_parameter_with_resolver(
                    &ty,
                    fallback_resolved,
                    resolver,
                )
            })
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            !known_builtin_type_name(&reference.name)
                && type_symbol_by_reference_name(resolved, &reference.name).is_none()
        }
        TypeExpr::Generic(generic) => generic.arguments.iter().any(|argument| {
            type_expr_contains_unresolved_type_parameter_with_resolver(
                argument,
                fallback_resolved,
                resolver,
            )
        }),
        TypeExpr::Pointer(pointer) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &pointer.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Borrow(borrow) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &borrow.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::View(view) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &view.element,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Array(array) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &array.element,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Optional(optional) => type_expr_contains_unresolved_type_parameter_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_unresolved_type_parameter_with_resolver(
                &fallible.success,
                fallback_resolved,
                resolver,
            ) || type_expr_contains_unresolved_type_parameter_with_resolver(
                &fallible.error,
                fallback_resolved,
                resolver,
            )
        }
    }
}
pub(in crate::driver::buildability) fn known_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "void"
            | "never"
            | "bool"
            | "str"
            | "error"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
    )
}
