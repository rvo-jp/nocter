use super::*;

pub(in crate::typecheck::returns) fn type_contains_borrow_like(
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    type_contains_borrow_like_inner(ty, resolved, &mut HashSet::new())
}

pub(in crate::typecheck::returns) fn type_contains_borrow_like_inner(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Str | Type::View { .. } => true,
        Type::Named(name) if name.starts_with('&') => true,
        Type::Named(name) => {
            type_symbol_contains_borrow_like(name, resolved, &HashMap::new(), resolving_names)
        }
        Type::Generic { name, arguments } => {
            let Some(symbol) = resolved.type_symbol_by_canonical_name(name) else {
                return false;
            };
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            type_symbol_contains_borrow_like(name, resolved, &substitutions, resolving_names)
        }
        Type::Array { element, .. } | Type::Optional(element) => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Fallible { success, error } => {
            type_contains_borrow_like_inner(success, resolved, resolving_names)
                || type_contains_borrow_like_inner(error, resolved, resolving_names)
        }
        Type::ArrayData { element } => {
            type_contains_borrow_like_inner(element, resolved, resolving_names)
        }
        Type::Error => true,
        Type::I32
        | Type::Primitive(_)
        | Type::StrData
        | Type::Void
        | Type::Never
        | Type::None
        | Type::Pointer(_)
        | Type::Parameter(_)
        | Type::Unresolved(_)
        | Type::Unknown => false,
    }
}

pub(in crate::typecheck::returns) fn type_symbol_contains_borrow_like(
    canonical_name: &str,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    if !resolving_names.insert(canonical_name.to_string()) {
        return false;
    }

    let result = resolved
        .type_symbol_by_canonical_name(canonical_name)
        .is_some_and(|symbol| match symbol.kind {
            TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
                type_expr_contains_borrow_like(target, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Struct => symbol.fields.iter().any(|field| {
                type_expr_contains_borrow_like(&field.ty, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Enum => symbol.variants.iter().any(|variant| {
                variant.payload.iter().any(|payload| {
                    type_expr_contains_borrow_like(
                        &payload.ty,
                        resolved,
                        substitutions,
                        resolving_names,
                    )
                })
            }),
            TypeSymbolKind::Interface => false,
        });

    resolving_names.remove(canonical_name);
    result
}

pub(in crate::typecheck::returns) fn type_expr_contains_borrow_like(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Borrow(_) => true,
        TypeExpr::View(view) => {
            type_expr_contains_borrow_like(&view.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_borrow_like(&array.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Optional(optional) => type_expr_contains_borrow_like(
            &optional.inner,
            resolved,
            substitutions,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_borrow_like(
                &fallible.success,
                resolved,
                substitutions,
                resolving_names,
            ) || type_expr_contains_borrow_like(
                &fallible.error,
                resolved,
                substitutions,
                resolving_names,
            )
        }
        TypeExpr::Pointer(_) => false,
        TypeExpr::Reference(reference) => {
            if reference.name == "error" {
                return true;
            }
            substitutions
                .get(&reference.name)
                .is_some_and(|ty| type_contains_borrow_like_inner(ty, resolved, resolving_names))
                || resolved
                    .type_symbol_by_reference_name(&reference.name)
                    .is_some_and(|symbol| {
                        type_symbol_contains_borrow_like(
                            &symbol.canonical_name,
                            resolved,
                            &HashMap::new(),
                            resolving_names,
                        )
                    })
        }
        TypeExpr::Generic(generic) => {
            if let Some(ty) = substitutions.get(&generic.name) {
                return type_contains_borrow_like_inner(ty, resolved, resolving_names);
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            let nested_substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().map(|argument| {
                    type_expr_to_type_with_substitutions(argument, resolved, None, substitutions)
                }))
                .collect();
            type_symbol_contains_borrow_like(
                &symbol.canonical_name,
                resolved,
                &nested_substitutions,
                resolving_names,
            )
        }
    }
}
