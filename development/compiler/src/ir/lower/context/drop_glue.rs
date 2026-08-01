use super::*;

pub(in crate::ir::lower) fn aggregate_drop_for_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<AggregateDrop>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    aggregate_drop_for_type_expr_with_resolver_ref(ty, root_source, fallback_resolved, &resolver)
}

pub(in crate::ir::lower) fn aggregate_drop_for_type_expr_with_resolver_ref<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<AggregateDrop>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    aggregate_drop_for_type_expr_inner(ty, root_source, fallback_resolved, resolver)
}

fn aggregate_drop_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<AggregateDrop>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let Some(array_drop) =
        array_drop_for_type_expr_with_resolver(ty, root_source, fallback_resolved, resolver)
    {
        return Some(AggregateDrop::Array(array_drop));
    }
    let direct =
        drop_glue_for_type_expr_with_resolver(ty, root_source, fallback_resolved, resolver);
    let fields = crate::ir::lower::aggregates::aggregate_fields_from_type_expr_with_resolver_ref(
        ty,
        root_source,
        fallback_resolved,
        resolver,
    )
    .map(|fields| struct_drop_fields(&fields))
    .unwrap_or_default();
    if !fields.is_empty() {
        return Some(AggregateDrop::Struct(StructDrop { direct, fields }));
    }
    if let Some(drop_glue) = direct {
        return Some(AggregateDrop::Direct(drop_glue));
    }

    payload_enum_drop_for_type_expr_with_resolver(ty, root_source, fallback_resolved, resolver)
        .map(AggregateDrop::PayloadEnum)
}

fn array_drop_for_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<ArrayDrop>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let element_ty = array_element_type_expr_with_resolver(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )?;
    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    let AbiType::Array { element, length } = value.ty else {
        return None;
    };
    let element_drop_kind =
        aggregate_drop_for_type_expr_inner(&element_ty, root_source, fallback_resolved, resolver)?;
    let element_layout = layout_of(&element).ok()?;
    let stride = array_element_stride(&element).ok()?;
    Some(ArrayDrop {
        length,
        stride,
        element_layout,
        element_drop_kind: Box::new(element_drop_kind),
    })
}

fn array_element_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Array(array) => Some(array.element.as_ref().clone()),
        TypeExpr::Reference(reference) => {
            let resolved = crate::ir::lower::aggregates::resolved_for_type_expr(
                ty,
                fallback_resolved,
                resolver,
            );
            let symbol = crate::ir::lower::aggregates::type_symbol_by_reference_name(
                resolved,
                &reference.name,
            )?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = array_element_type_expr_with_resolver(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Generic(generic) => {
            let resolved = crate::ir::lower::aggregates::resolved_for_type_expr(
                ty,
                fallback_resolved,
                resolver,
            );
            let symbol = crate::ir::lower::aggregates::type_symbol_by_reference_name(
                resolved,
                &generic.name,
            )?;
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
            let result = array_element_type_expr_with_resolver(
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

fn struct_drop_fields(fields: &[AggregateField]) -> Vec<StructDropField> {
    fields
        .iter()
        .filter(|field| !field.name.contains('.'))
        .filter_map(struct_drop_field)
        .collect()
}

fn struct_drop_field(field: &AggregateField) -> Option<StructDropField> {
    let layout = field.kind.copy_aggregate_layout()?;
    let drop_kind = field.drop_kind.clone()?;
    Some(StructDropField {
        offset: field.offset,
        layout,
        drop_kind: Box::new(drop_kind),
    })
}

fn payload_enum_drop_for_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<PayloadEnumDrop>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    let AbiType::Enum(enum_) = value.ty else {
        return None;
    };
    let (symbol, substitutions) =
        payload_enum_symbol_and_substitutions_for_type_expr(ty, fallback_resolved, resolver)?;
    let payload_offset = u32::try_from(enum_.payload_offset).ok()?;
    let mut variants = Vec::new();
    for variant in &symbol.variants {
        let abi_variant = enum_
            .variants
            .iter()
            .find(|abi_variant| abi_variant.name == variant.name)?;
        match payload_enum_drop_variant_for_payload(
            variant,
            abi_variant,
            payload_offset,
            root_source,
            fallback_resolved,
            resolver,
            &substitutions,
        ) {
            Ok(Some(variant)) => variants.push(variant),
            Ok(None) => {}
            Err(()) => return None,
        }
    }
    (!variants.is_empty()).then_some(PayloadEnumDrop { variants })
}

fn payload_enum_drop_variant_for_payload<'a, F>(
    variant: &crate::resolve::EnumVariantSignature,
    abi_variant: &crate::abi::AbiEnumVariant,
    payload_offset: u32,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> Result<Option<PayloadEnumDropVariant>, ()>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match variant.payload.as_slice() {
        [] => Ok(None),
        [payload] => {
            let Some(field) = payload_enum_drop_field_for_payload(
                &payload.ty,
                abi_variant.payload.as_ref().ok_or(())?,
                payload_offset,
                root_source,
                fallback_resolved,
                resolver,
                substitutions,
            )?
            else {
                return Ok(None);
            };
            Ok(Some(PayloadEnumDropVariant {
                tag: abi_variant.tag,
                fields: vec![field],
            }))
        }
        payloads => {
            let Some(AbiType::Struct(abi_fields)) = abi_variant.payload.as_ref() else {
                return Err(());
            };
            if payloads.len() != abi_fields.len() {
                return Err(());
            }
            let layout = layout_struct(abi_fields).map_err(|_| ())?;
            if payloads.len() != layout.fields.len() {
                return Err(());
            }

            let mut fields = Vec::new();
            for ((payload, abi_field), field_layout) in payloads
                .iter()
                .zip(abi_fields.iter())
                .zip(layout.fields.iter())
            {
                let field_offset = payload_offset
                    .checked_add(u32::try_from(field_layout.offset).map_err(|_| ())?)
                    .ok_or(())?;
                if let Some(field) = payload_enum_drop_field_for_payload(
                    &payload.ty,
                    &abi_field.ty,
                    field_offset,
                    root_source,
                    fallback_resolved,
                    resolver,
                    substitutions,
                )? {
                    fields.push(field);
                }
            }
            if fields.is_empty() {
                return Ok(None);
            }
            Ok(Some(PayloadEnumDropVariant {
                tag: abi_variant.tag,
                fields,
            }))
        }
    }
}

fn payload_enum_drop_field_for_payload<'a, F>(
    payload_ty: &TypeExpr,
    payload_abi: &AbiType,
    payload_offset: u32,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> Result<Option<PayloadEnumDropField>, ()>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let ty = substitute_type_expr_parameters(payload_ty, substitutions);
    let Some(drop_kind) =
        aggregate_drop_for_type_expr_inner(&ty, root_source, fallback_resolved, resolver)
    else {
        return Ok(None);
    };
    let payload_layout = layout_of(payload_abi).map_err(|_| ())?;
    Ok(Some(PayloadEnumDropField {
        payload_offset,
        payload_layout,
        drop_kind: Box::new(drop_kind),
    }))
}

fn payload_enum_symbol_and_substitutions_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    payload_enum_symbol_and_substitutions_for_type_expr_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

fn payload_enum_symbol_and_substitutions_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    match ty {
        TypeExpr::Reference(reference) => {
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            match symbol.kind {
                TypeSymbolKind::Enum if symbol.generic_arity == 0 => Some((symbol, HashMap::new())),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Generic(generic) => {
            let symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions: HashMap<String, TypeExpr> = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Enum => Some((symbol, substitutions)),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        &target,
                        fallback_resolved,
                        resolver,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

pub(in crate::ir::lower) fn drop_glue_for_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<DropGlue>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    drop_glue_for_type_expr_inner(
        ty,
        root_source,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
        true,
    )
}

fn drop_glue_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
    peel_tagged_payloads: bool,
) -> Option<DropGlue>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Fallible(fallible) if peel_tagged_payloads => {
            return drop_glue_for_type_expr_inner(
                &fallible.success,
                root_source,
                fallback_resolved,
                resolver,
                resolving_names,
                peel_tagged_payloads,
            );
        }
        TypeExpr::Optional(optional) if peel_tagged_payloads => {
            return drop_glue_for_type_expr_inner(
                &optional.inner,
                root_source,
                fallback_resolved,
                resolver,
                resolving_names,
                peel_tagged_payloads,
            );
        }
        _ => {}
    }

    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let type_symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if type_symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = type_symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            (generic.name.as_str(), substitutions)
        }
        _ => return None,
    };
    let (symbol, type_symbol) = type_symbol_definition_by_reference_name(resolved, type_name)?;
    if type_symbol.kind == TypeSymbolKind::Alias {
        let target = type_symbol.alias_target.as_ref()?;
        if !resolving_names.insert(type_symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let drop_glue = drop_glue_for_type_expr_inner(
            &target,
            root_source,
            fallback_resolved,
            resolver,
            resolving_names,
            peel_tagged_payloads,
        );
        resolving_names.remove(&type_symbol.canonical_name);
        return drop_glue;
    }

    let drop_member = type_symbol.drop_member.as_ref()?;
    let target_name = if type_symbol.generic_arity > 0 {
        drop_target_name_from_base_and_type_expr(&drop_member.target_name, ty)
    } else {
        drop_member.target_name.clone()
    };
    let target = if symbol.declaration_span.source == root_source {
        CallTarget::same_file(target_name)
    } else {
        CallTarget::imported(symbol.declaration_span.source, target_name)
    };
    Some(DropGlue { target })
}

pub(super) fn enum_variant_index(symbol: &TypeSymbol, variant_name: &str) -> Option<u8> {
    let index = symbol
        .variants
        .iter()
        .position(|variant| variant.name == variant_name)?;
    u8::try_from(index).ok()
}

pub(super) fn payloadless_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payloadless,
    )
}

pub(super) fn payload_enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr(
        ty,
        fallback_resolved,
        resolver,
        EnumPayloadRequirement::Payload,
    )
}

#[derive(Clone, Copy)]
enum EnumPayloadRequirement {
    Payloadless,
    Payload,
}

fn enum_symbol_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    enum_symbol_for_type_expr_inner(
        ty,
        fallback_resolved,
        resolver,
        payload_requirement,
        &mut HashSet::new(),
    )
}

fn enum_symbol_for_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    payload_requirement: EnumPayloadRequirement,
    resolving_names: &mut HashSet<String>,
) -> Option<&'a TypeSymbol>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let (type_name, substitutions) = match ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let type_symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            if type_symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = type_symbol
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
        | TypeExpr::Fallible(_) => return None,
    };
    let type_symbol = type_symbol_by_reference_name(resolved, type_name)?;
    if type_symbol.kind == TypeSymbolKind::Alias {
        let target = type_symbol.alias_target.as_ref()?;
        if !resolving_names.insert(type_symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let symbol = enum_symbol_for_type_expr_inner(
            &target,
            fallback_resolved,
            resolver,
            payload_requirement,
            resolving_names,
        );
        resolving_names.remove(&type_symbol.canonical_name);
        return symbol;
    }

    enum_symbol_matches_payload_requirement(type_symbol, payload_requirement).then_some(type_symbol)
}

fn enum_symbol_matches_payload_requirement(
    symbol: &TypeSymbol,
    payload_requirement: EnumPayloadRequirement,
) -> bool {
    if symbol.kind != TypeSymbolKind::Enum || symbol.variants.len() > 256 {
        return false;
    }

    match payload_requirement {
        EnumPayloadRequirement::Payloadless => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        EnumPayloadRequirement::Payload => symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty()),
    }
}

fn resolved_for_type_expr<'a, F>(
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

fn type_expr_symbol_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn type_symbol_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        short_qualified_type_name(name)
            .and_then(|short| resolved.type_symbol_by_reference_name(short))
    })
}

fn type_symbol_definition_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<(&'a Symbol, &'a TypeSymbol)> {
    resolved
        .type_symbol_definition_by_reference_name(name)
        .or_else(|| {
            short_qualified_type_name(name)
                .and_then(|short| resolved.type_symbol_definition_by_reference_name(short))
        })
}

fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}

fn drop_target_name_from_base_and_type_expr(base_target_name: &str, ty: &TypeExpr) -> String {
    let Some(base_type_name) = base_target_name.strip_suffix(".drop") else {
        return base_target_name.to_string();
    };
    let TypeExpr::Generic(generic) = ty else {
        return base_target_name.to_string();
    };
    let arguments = generic
        .arguments
        .iter()
        .map(type_expr_display_lossy)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base_type_name}<{arguments}>.drop")
}
