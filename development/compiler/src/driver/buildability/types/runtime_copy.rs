use super::*;

pub(in crate::driver::buildability) fn type_expr_is_runtime_copy_struct_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            if symbol.generic_arity > 0 {
                return false;
            }
            type_symbol_is_runtime_copy_struct_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &HashMap::new(),
                resolving_names,
            )
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
                return false;
            };
            if symbol.generic_arity != generic.arguments.len() {
                return false;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            type_symbol_is_runtime_copy_struct_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Fallible(fallible) => type_expr_is_runtime_copy_struct_with_resolver(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_runtime_copy_struct_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        _ => false,
    }
}
pub(in crate::driver::buildability) fn type_symbol_is_runtime_copy_struct_with_resolver<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }

    let is_copy = match symbol.kind {
        TypeSymbolKind::Struct if !symbol.is_copy => false,
        TypeSymbolKind::Struct => symbol.fields.iter().all(|field| {
            let field_ty = substitute_type_expr_parameters(&field.ty, substitutions);
            type_expr_is_runtime_copy_value_with_resolver(
                &field_ty,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }),
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
            let target = substitute_type_expr_parameters(target, substitutions);
            type_expr_is_runtime_copy_struct_with_resolver(
                &target,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }),
        TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
    };

    resolving_names.remove(&symbol.canonical_name);
    is_copy
}
pub(in crate::driver::buildability) fn type_expr_is_runtime_copy_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Callable(_) => false,
        TypeExpr::Opaque(_) => false,
        TypeExpr::Projection(_) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            crate::typecheck::normalize_associated_type_expr(ty, resolved).is_some_and(|ty| {
                type_expr_is_runtime_copy_value_with_resolver(
                    &ty,
                    fallback_resolved,
                    resolver,
                    resolving_names,
                )
            })
        }
        TypeExpr::Closure(closure) => closure.captures.iter().all(|capture| match capture.mode {
            crate::ast::ClosureCaptureMode::ReadonlyBorrow => true,
            crate::ast::ClosureCaptureMode::ReadwriteBorrow => false,
            crate::ast::ClosureCaptureMode::Move => type_expr_is_runtime_copy_value_with_resolver(
                &capture.ty,
                fallback_resolved,
                resolver,
                resolving_names,
            ),
        }),
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
            | "isize" | "error" => true,
            "str" | "void" | "never" | "Self" => false,
            _ => {
                let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
                let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                    return false;
                };
                if symbol.generic_arity > 0 {
                    return false;
                }
                type_symbol_is_runtime_copy_value_with_resolver(
                    symbol,
                    fallback_resolved,
                    resolver,
                    &HashMap::new(),
                    resolving_names,
                )
            }
        },
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
                return false;
            };
            if symbol.generic_arity != generic.arguments.len() {
                return false;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            type_symbol_is_runtime_copy_value_with_resolver(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Borrow(borrow) => !borrow.is_readwrite,
        TypeExpr::Pointer(_) => true,
        TypeExpr::Array(array) => type_expr_is_runtime_copy_value_with_resolver(
            &array.element,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_runtime_copy_value_with_resolver(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_is_runtime_copy_value_with_resolver(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            ) && type_expr_is_runtime_copy_value_with_resolver(
                &fallible.error,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::View(_) => false,
    }
}
pub(in crate::driver::buildability) fn type_symbol_is_runtime_copy_value_with_resolver<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match symbol.kind {
        TypeSymbolKind::Struct => type_symbol_is_runtime_copy_struct_with_resolver(
            symbol,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        ),
        TypeSymbolKind::Enum => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        TypeSymbolKind::Alias => {
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let is_copy = symbol.alias_target.as_ref().is_some_and(|target| {
                let target = substitute_type_expr_parameters(target, substitutions);
                type_expr_is_runtime_copy_value_with_resolver(
                    &target,
                    fallback_resolved,
                    resolver,
                    resolving_names,
                )
            });
            resolving_names.remove(&symbol.canonical_name);
            is_copy
        }
        TypeSymbolKind::Interface => false,
    }
}
