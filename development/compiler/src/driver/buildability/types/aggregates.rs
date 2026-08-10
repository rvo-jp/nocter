use super::*;

pub(in crate::driver::buildability) fn type_expr_is_supported_aggregate_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, &source_resolver)
}
pub(in crate::driver::buildability) fn type_expr_is_supported_aggregate_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver) else {
        return false;
    };
    if matches!(ty, TypeExpr::Closure(_)) {
        return matches!(value.ty, AbiType::Struct(_));
    }
    match &value.ty {
        AbiType::Enum(_) => {
            type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, resolver)
        }
        _ => abi_value_is_supported_aggregate_value(&value),
    }
}
pub(in crate::driver::buildability) fn abi_value_is_supported_aggregate_value(
    value: &AbiValue,
) -> bool {
    match &value.ty {
        AbiType::Struct(_) => value.layout.size > 0,
        AbiType::Array { element, .. } => fixed_array_element_abi_is_buildable(element),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn type_expr_is_supported_payload_enum_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, &source_resolver)
}
pub(in crate::driver::buildability) fn type_expr_is_supported_payload_enum_value_with_resolver<
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
    type_expr_is_supported_payload_enum_value_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}
pub(in crate::driver::buildability) fn type_expr_is_supported_payload_enum_value_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Callable(_) | TypeExpr::Closure(_) => false,
        TypeExpr::Projection(_) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            crate::typecheck::normalize_associated_type_expr(ty, resolved).is_some_and(|ty| {
                type_expr_is_supported_payload_enum_value_inner(
                    &ty,
                    fallback_resolved,
                    resolver,
                    resolving_names,
                )
            })
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            match symbol.kind {
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let result = type_expr_is_supported_payload_enum_value_inner(
                        target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Enum if symbol.generic_arity == 0 => {
                    type_symbol_payload_enum_payloads_are_supported_values(
                        symbol,
                        fallback_resolved,
                        resolver,
                        &HashMap::new(),
                    )
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
                return false;
            };
            if symbol.generic_arity != generic.arguments.len() {
                return false;
            }
            let substitutions: HashMap<String, TypeExpr> = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Alias => {
                    let Some(target) = &symbol.alias_target else {
                        return false;
                    };
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return false;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let result = type_expr_is_supported_payload_enum_value_inner(
                        &target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Enum => type_symbol_payload_enum_payloads_are_supported_values(
                    symbol,
                    fallback_resolved,
                    resolver,
                    &substitutions,
                ),
                TypeSymbolKind::Struct | TypeSymbolKind::Interface => false,
            }
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => false,
    }
}
pub(in crate::driver::buildability) fn type_symbol_payload_enum_payloads_are_supported_values<
    'a,
    F,
>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if symbol.kind != TypeSymbolKind::Enum
        || symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return false;
    }

    symbol.variants.iter().all(|variant| {
        payload_enum_variant_payloads_are_supported(
            &variant.payload,
            fallback_resolved,
            resolver,
            substitutions,
        )
    })
}
pub(in crate::driver::buildability) fn type_expr_has_supported_recursive_drop_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (symbol, substitutions) = match ty {
        TypeExpr::Callable(_) => return false,
        TypeExpr::Projection(_) => {
            let Some(normalized) = crate::typecheck::normalize_associated_type_expr(ty, resolved)
            else {
                return false;
            };
            return type_expr_has_supported_recursive_drop_with_resolver(
                &normalized,
                fallback_resolved,
                resolver,
                resolving_names,
            );
        }
        TypeExpr::Closure(closure) => {
            return closure.captures.iter().all(|capture| {
                capture.mode != crate::ast::ClosureCaptureMode::Move
                    || type_expr_has_supported_recursive_drop_with_resolver(
                        &capture.ty,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    )
                    || crate::abi::abi_value_from_type_expr_with_resolver(
                        &capture.ty,
                        fallback_resolved,
                        resolver,
                    )
                    .is_ok()
            });
        }
        TypeExpr::Reference(reference) => {
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return false;
            };
            (symbol, HashMap::new())
        }
        TypeExpr::Generic(generic) => {
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
            (symbol, substitutions)
        }
        TypeExpr::Array(_) => {
            return type_expr_is_supported_recursive_drop_array_with_resolver(
                ty,
                fallback_resolved,
                resolver,
                resolving_names,
            );
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => return false,
    };

    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }
    let result = match symbol.kind {
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
            let target = substitute_type_expr_parameters(target, &substitutions);
            type_expr_has_supported_recursive_drop_with_resolver(
                &target,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }),
        TypeSymbolKind::Struct => {
            let mut has_drop = symbol.destructor.is_some();
            let fields_are_supported = symbol.fields.iter().all(|field| {
                let field_ty = substitute_type_expr_parameters(&field.ty, &substitutions);
                if type_expr_is_runtime_copy_value_with_resolver(
                    &field_ty,
                    fallback_resolved,
                    resolver,
                    &mut HashSet::new(),
                ) {
                    true
                } else if type_expr_has_supported_recursive_drop_with_resolver(
                    &field_ty,
                    fallback_resolved,
                    resolver,
                    resolving_names,
                ) {
                    has_drop = true;
                    true
                } else {
                    false
                }
            });
            has_drop && fields_are_supported
        }
        TypeSymbolKind::Enum => {
            let mut has_drop = false;
            let payloads_are_supported = symbol.variants.iter().all(|variant| {
                variant.payload.iter().all(|payload| {
                    let payload_ty = substitute_type_expr_parameters(&payload.ty, &substitutions);
                    if type_expr_is_runtime_copy_value_with_resolver(
                        &payload_ty,
                        fallback_resolved,
                        resolver,
                        &mut HashSet::new(),
                    ) {
                        true
                    } else if type_expr_has_supported_recursive_drop_with_resolver(
                        &payload_ty,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    ) {
                        has_drop = true;
                        true
                    } else {
                        false
                    }
                })
            });
            has_drop && payloads_are_supported
        }
        TypeSymbolKind::Interface => false,
    };
    resolving_names.remove(&symbol.canonical_name);
    result
}

fn type_expr_is_supported_recursive_drop_array_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver) else {
        return false;
    };
    let AbiType::Array { element, .. } = &value.ty else {
        return false;
    };
    if !matches!(element.as_ref(), AbiType::Struct(_)) {
        return false;
    }
    let Some(element_ty) = fixed_array_element_type_expr_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    ) else {
        return false;
    };
    type_expr_is_supported_aggregate_value_with_resolver(&element_ty, fallback_resolved, resolver)
        && type_expr_has_supported_recursive_drop_with_resolver(
            &element_ty,
            fallback_resolved,
            resolver,
            resolving_names,
        )
}

pub(in crate::driver::buildability) fn fixed_array_element_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Callable(_) | TypeExpr::Closure(_) => None,
        TypeExpr::Projection(_) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let normalized = crate::typecheck::normalize_associated_type_expr(ty, resolved)?;
            fixed_array_element_type_expr_with_resolver(
                &normalized,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::Array(array) => Some(array.element.as_ref().clone()),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = fixed_array_element_type_expr_with_resolver(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            let target = substitute_type_expr_parameters(target, &substitutions);
            let result = fixed_array_element_type_expr_with_resolver(
                &target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

pub(in crate::driver::buildability) fn type_expr_is_supported_move_only_fixed_array_with_resolver<
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
    type_expr_is_supported_recursive_drop_array_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

pub(in crate::driver::buildability) fn type_expr_is_supported_fixed_array_aggregate_with_resolver<
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
    let Ok(value) = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver) else {
        return false;
    };
    let AbiType::Array { element, .. } = value.ty else {
        return false;
    };
    if !matches!(element.as_ref(), AbiType::Struct(_)) {
        return false;
    }
    let Some(element_ty) = fixed_array_element_type_expr_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    ) else {
        return false;
    };
    type_expr_is_supported_aggregate_value_with_resolver(&element_ty, fallback_resolved, resolver)
        || type_expr_has_supported_recursive_drop_with_resolver(
            &element_ty,
            fallback_resolved,
            resolver,
            &mut HashSet::new(),
        )
}

pub(in crate::driver::buildability) fn type_expr_is_supported_aggregate_return_with_resolver<
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
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_move_only_fixed_array_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        )
}
