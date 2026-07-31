use super::*;

pub(super) fn type_expr_has_str_view_abi_with_resolver<'a, F>(
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

pub(super) fn binding_type_expr_with_substitutions(
    statement: &BindingStmt,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    statement
        .ty
        .clone()
        .or_else(|| {
            typecheck_facts
                .binding_type_expr(statement.name_span)
                .cloned()
        })
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}

pub(super) fn local_identifier_type_expr_with_substitutions(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let symbol = resolved.local_symbol_for_identifier(identifier)?;
    typecheck_facts
        .binding_type_expr(symbol.name_span)
        .cloned()
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}

pub(super) fn type_expr_is_known_unsupported_scalar_value_for_sources(
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

pub(super) fn type_expr_is_known_unsupported_scalar_value_with_resolver<'a, F>(
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

pub(super) fn type_expr_is_known_unsupported_scalar_value_inner<'a, F>(
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

pub(super) fn resolved_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> &'a ResolveOutput
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let source_resolved = resolver(ty.span().source);
    let Some(name) = type_expr_symbol_name(ty) else {
        return source_resolved.unwrap_or(fallback_resolved);
    };

    if let Some(resolved) = source_resolved
        && type_symbol_by_reference_name(resolved, name).is_some()
    {
        return resolved;
    }
    if type_symbol_by_reference_name(fallback_resolved, name).is_some() {
        return fallback_resolved;
    }

    source_resolved.unwrap_or(fallback_resolved)
}

pub(super) fn type_expr_symbol_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

pub(super) fn type_symbol_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        short_qualified_type_name(name)
            .and_then(|short| resolved.type_symbol_by_reference_name(short))
    })
}

pub(super) fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}

pub(super) fn type_expr_is_top_level_optional_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_top_level_optional_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_top_level_optional_success_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_top_level_optional_success_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_top_level_optional_success_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(optional) => Some(optional.inner.as_ref().clone()),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = type_expr_top_level_optional_success_inner(
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

pub(super) fn type_expr_is_top_level_optional_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(_) => true,
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
            let result = type_expr_is_top_level_optional_inner(
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

pub(super) fn type_expr_is_buildable_scalar_or_view_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn type_expr_is_buildable_scalar_or_view_with_resolver<'a, F>(
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

pub(super) fn type_expr_is_buildable_scalar_or_view_inner<'a, F>(
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

pub(super) fn type_expr_has_buildable_scalar_abi_with_resolver<'a, F>(
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

pub(super) fn type_expr_resolves_to_str_with_resolver<'a, F>(
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

pub(super) fn type_expr_resolves_to_builtin_reference_inner<'a, F>(
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

pub(super) fn type_expr_resolves_to_view_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_view_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_resolves_to_view_inner<'a, F>(
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

pub(super) fn type_expr_resolves_to_supported_slice_view_with_resolver<'a, F>(
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

pub(super) fn type_expr_resolves_to_supported_slice_view_inner<'a, F>(
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

pub(super) fn type_expr_resolved_view_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolved_view_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_resolved_view_element_kind_inner<'a, F>(
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

pub(super) fn type_expr_is_error_parameter_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_error_parameter_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn type_expr_is_error_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_error_parameter_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_is_error_parameter_inner<'a, F>(
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

pub(super) fn type_expr_is_supported_borrow_parameter_with_resolver<'a, F>(
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
        matches!(
            value.ty,
            AbiType::I32
                | AbiType::U8
                | AbiType::Usize
                | AbiType::Bool
                | AbiType::Pointer
                | AbiType::Struct(_)
        )
    })
}

pub(super) fn type_expr_is_supported_aggregate_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn type_expr_is_supported_aggregate_value_with_resolver<'a, F>(
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
    match &value.ty {
        AbiType::Enum(_) => {
            type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, resolver)
        }
        _ => abi_value_is_supported_aggregate_value(&value),
    }
}

pub(super) fn abi_value_is_supported_aggregate_value(value: &AbiValue) -> bool {
    match &value.ty {
        AbiType::Struct(_) => value.layout.size > 0 && !abi_type_contains_enum(&value.ty),
        AbiType::Array { element, .. } => {
            fixed_array_element_abi_is_buildable(element) && !abi_type_contains_enum(element)
        }
        _ => false,
    }
}

pub(super) fn abi_type_contains_enum(ty: &AbiType) -> bool {
    match ty {
        AbiType::Enum(_) => true,
        AbiType::Array { element, .. } => abi_type_contains_enum(element),
        AbiType::Struct(fields) => fields.iter().any(|field| abi_type_contains_enum(&field.ty)),
        AbiType::Bool
        | AbiType::U8
        | AbiType::I8
        | AbiType::U16
        | AbiType::I16
        | AbiType::U32
        | AbiType::I32
        | AbiType::U64
        | AbiType::I64
        | AbiType::Usize
        | AbiType::Isize
        | AbiType::Pointer
        | AbiType::Borrow
        | AbiType::StrView
        | AbiType::SliceView => false,
    }
}

pub(super) fn type_expr_is_supported_payload_enum_value_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_payload_enum_value_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn type_expr_is_supported_payload_enum_value_with_resolver<'a, F>(
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

pub(super) fn type_expr_is_supported_payload_enum_value_inner<'a, F>(
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

pub(super) fn type_symbol_payload_enum_payloads_are_supported_values<'a, F>(
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

pub(super) fn type_expr_has_direct_drop_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
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
            (generic.name.as_str(), substitutions)
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => return false,
    };

    let Some(symbol) = type_symbol_by_reference_name(resolved, type_name) else {
        return false;
    };
    if symbol.kind == TypeSymbolKind::Alias {
        let Some(target) = symbol.alias_target.as_ref() else {
            return false;
        };
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return false;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let has_drop = type_expr_has_direct_drop_with_resolver(
            &target,
            fallback_resolved,
            resolver,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return has_drop;
    }

    symbol.drop_member.is_some()
}

pub(super) fn type_expr_is_supported_aggregate_return_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
}

pub(super) fn type_expr_contains_unresolved_type_parameter(
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

pub(super) fn type_expr_contains_unresolved_type_parameter_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
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

pub(super) fn known_builtin_type_name(name: &str) -> bool {
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

pub(super) fn type_expr_is_supported_slice_index_element_with_resolver<'a, F>(
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

pub(super) fn type_expr_is_supported_std_vec_element_storage(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if type_expr_slice_element_kind_with_resolver(ty, fallback_resolved, &source_resolver)
        != TypecheckSliceElementKind::Other
    {
        return true;
    }

    type_expr_is_supported_copy_aggregate_vec_element_with_resolver(
        ty,
        fallback_resolved,
        &source_resolver,
    )
}

pub(super) fn type_expr_is_supported_copy_aggregate_vec_element_with_resolver<'a, F>(
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
    if !matches!(value.ty, AbiType::Struct(_)) || value.layout.size == 0 {
        return false;
    }
    type_expr_is_runtime_copy_struct_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

pub(super) fn type_expr_is_runtime_copy_struct_with_resolver<'a, F>(
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

pub(super) fn type_symbol_is_runtime_copy_struct_with_resolver<'a, F>(
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

pub(super) fn type_expr_is_runtime_copy_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
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

pub(super) fn type_symbol_is_runtime_copy_value_with_resolver<'a, F>(
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

pub(super) fn type_expr_slice_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_slice_element_kind_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_slice_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => TypecheckSliceElementKind::I32,
        TypeExpr::Reference(reference) if reference.name == "u8" => TypecheckSliceElementKind::U8,
        TypeExpr::Reference(reference) if reference.name == "usize" => {
            TypecheckSliceElementKind::Usize
        }
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

pub(super) fn type_expr_resolves_to_borrow_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_resolves_to_borrow_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn type_expr_resolves_to_borrow_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(_) => true,
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
            let resolves = type_expr_resolves_to_borrow_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            resolves
        }
        _ => false,
    }
}

pub(super) fn type_expr_fallible_depth(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> usize {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_fallible_depth_inner(ty, fallback_resolved, &source_resolver, &mut HashSet::new())
}

pub(super) fn type_expr_fallible_depth_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> usize
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return 0;
            };
            let Some(target) = &symbol.alias_target else {
                return 0;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return 0;
            }
            let depth = type_expr_fallible_depth_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            depth
        }
        TypeExpr::Fallible(fallible) => {
            1 + type_expr_fallible_depth_inner(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::Optional(optional) => {
            1 + type_expr_fallible_depth_inner(
                &optional.inner,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        _ => 0,
    }
}
