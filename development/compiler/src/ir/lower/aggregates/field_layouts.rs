use super::*;

pub(in crate::ir::lower) fn aggregate_fields_from_type_expr(
    ty: &TypeExpr,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Option<Vec<AggregateField>> {
    aggregate_fields_from_type_expr_with_resolver(ty, root_source, resolved, |_| Some(resolved))
}

pub(in crate::ir::lower) fn aggregate_fields_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Vec<AggregateField>>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    aggregate_fields_from_type_expr_with_resolver_ref(ty, root_source, fallback_resolved, &resolver)
}

pub(in crate::ir::lower) fn aggregate_fields_from_type_expr_with_resolver_ref<'a, F>(
    ty: &TypeExpr,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<Vec<AggregateField>>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let ty = match ty {
        TypeExpr::Fallible(fallible) => &fallible.success,
        TypeExpr::Optional(optional) => &optional.inner,
        _ => ty,
    };
    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    let AbiType::Struct(fields) = value.ty else {
        return Some(Vec::new());
    };
    let struct_layout = layout_struct(&fields).ok()?;
    let source_fields = struct_field_signatures_from_type_expr(ty, fallback_resolved, resolver)?;
    if fields.len() != source_fields.len() {
        return None;
    }

    let mut aggregate_fields = Vec::new();
    for ((field, layout), source_field) in fields
        .iter()
        .zip(struct_layout.fields.iter())
        .zip(source_fields.iter())
    {
        collect_aggregate_fields(
            &field.name,
            &field.ty,
            Some(&source_field.ty),
            layout.offset,
            root_source,
            fallback_resolved,
            resolver,
            &mut aggregate_fields,
        )?;
    }
    Some(aggregate_fields)
}

fn struct_field_signatures_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<Vec<StructFieldSignature>>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            if symbol.generic_arity > 0 {
                return None;
            }
            match symbol.kind {
                TypeSymbolKind::Struct => Some(symbol.fields.clone()),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    struct_field_signatures_from_type_expr(target, fallback_resolved, resolver)
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &generic.name)?;
            let substitutions = generic_type_expr_substitutions(symbol, ty)?;
            match symbol.kind {
                TypeSymbolKind::Struct => Some(
                    symbol
                        .fields
                        .iter()
                        .cloned()
                        .map(|mut field| {
                            field.ty = substitute_type_expr_parameters(&field.ty, &substitutions);
                            field
                        })
                        .collect(),
                ),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    struct_field_signatures_from_type_expr(&target, fallback_resolved, resolver)
                }
                TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Fallible(fallible) => {
            struct_field_signatures_from_type_expr(&fallible.success, fallback_resolved, resolver)
        }
        TypeExpr::Optional(optional) => {
            struct_field_signatures_from_type_expr(&optional.inner, fallback_resolved, resolver)
        }
        _ => None,
    }
}

pub(super) fn generic_type_expr_substitutions(
    symbol: &TypeSymbol,
    ty: &TypeExpr,
) -> Option<HashMap<String, TypeExpr>> {
    let TypeExpr::Generic(generic) = ty else {
        return None;
    };
    if symbol.generic_arity != generic.arguments.len() {
        return None;
    }
    Some(
        symbol
            .generic_parameters
            .iter()
            .cloned()
            .zip(generic.arguments.iter().cloned())
            .collect(),
    )
}

pub(in crate::ir::lower) fn resolved_for_type_expr<'a, F>(
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

pub(in crate::ir::lower) fn type_symbol_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        short_qualified_type_name(name)
            .and_then(|short| resolved.type_symbol_by_reference_name(short))
    })
}

fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}

fn collect_aggregate_fields<'a, F>(
    name: &str,
    ty: &AbiType,
    source_ty: Option<&TypeExpr>,
    base_offset: u64,
    root_source: SourceId,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    aggregate_fields: &mut Vec<AggregateField>,
) -> Option<()>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let Some(kind) =
        aggregate_field_kind_from_abi_type(ty, source_ty, fallback_resolved, resolver)
    {
        let offset = u32::try_from(base_offset).ok()?;
        let is_copy = !matches!(kind, AggregateFieldKind::Array { .. })
            || source_ty.is_some_and(|ty| {
                type_expr_is_copy_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
            });
        let drop_kind = source_ty.and_then(|ty| {
            aggregate_drop_for_type_expr_with_resolver_ref(
                ty,
                root_source,
                fallback_resolved,
                resolver,
            )
        });
        aggregate_fields.push(AggregateField {
            name: name.to_string(),
            offset,
            kind,
            is_copy,
            drop_kind,
        });
        return Some(());
    }

    let AbiType::Struct(fields) = ty else {
        return Some(());
    };
    let struct_layout = layout_struct(fields).ok()?;
    let offset = u32::try_from(base_offset).ok()?;
    let mut nested_fields = Vec::new();
    let nested_source_fields = if let Some(source_ty) = source_ty {
        let source_fields =
            struct_field_signatures_from_type_expr(source_ty, fallback_resolved, resolver)?;
        if fields.len() != source_fields.len() {
            return None;
        }
        Some(source_fields)
    } else {
        None
    };
    for (index, (field, layout)) in fields.iter().zip(struct_layout.fields.iter()).enumerate() {
        let nested_source_ty = nested_source_fields
            .as_ref()
            .and_then(|source_fields| source_fields.get(index))
            .map(|field| &field.ty);
        collect_aggregate_fields(
            &field.name,
            &field.ty,
            nested_source_ty,
            layout.offset,
            root_source,
            fallback_resolved,
            resolver,
            &mut nested_fields,
        )?;
    }
    aggregate_fields.push(AggregateField {
        name: name.to_string(),
        offset,
        kind: AggregateFieldKind::Aggregate {
            layout: ValueLayout::new(struct_layout.size, struct_layout.align),
            fields: nested_fields,
        },
        is_copy: source_ty.is_some_and(|ty| {
            type_expr_is_copy_struct_with_resolver(ty, fallback_resolved, resolver)
        }),
        drop_kind: source_ty.and_then(|ty| {
            aggregate_drop_for_type_expr_with_resolver_ref(
                ty,
                root_source,
                fallback_resolved,
                resolver,
            )
        }),
    });

    for (index, (field, layout)) in fields.iter().zip(struct_layout.fields.iter()).enumerate() {
        let offset = base_offset.checked_add(layout.offset)?;
        let nested_source_ty = nested_source_fields
            .as_ref()
            .and_then(|source_fields| source_fields.get(index))
            .map(|field| &field.ty);
        collect_aggregate_fields(
            &format!("{name}.{}", field.name),
            &field.ty,
            nested_source_ty,
            offset,
            root_source,
            fallback_resolved,
            resolver,
            aggregate_fields,
        )?;
    }
    Some(())
}

fn aggregate_field_kind_from_abi_type<'a, F>(
    ty: &AbiType,
    source_ty: Option<&TypeExpr>,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<AggregateFieldKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        AbiType::I8 => Some(AggregateFieldKind::I8),
        AbiType::I16 => Some(AggregateFieldKind::I16),
        AbiType::I32 => Some(AggregateFieldKind::I32),
        AbiType::I64 => Some(AggregateFieldKind::I64),
        AbiType::Isize => Some(AggregateFieldKind::Isize),
        AbiType::U16 => Some(AggregateFieldKind::U16),
        AbiType::U32 => Some(AggregateFieldKind::U32),
        AbiType::U64 => Some(AggregateFieldKind::U64),
        AbiType::U8 => Some(AggregateFieldKind::U8),
        AbiType::Bool => Some(AggregateFieldKind::Bool),
        AbiType::Usize | AbiType::Pointer => Some(AggregateFieldKind::Usize),
        AbiType::StrView => Some(AggregateFieldKind::Str),
        AbiType::Array { element, length } => {
            let stride = array_element_stride(element).ok()?;
            Some(AggregateFieldKind::Array {
                layout: layout_of(ty).ok()?,
                element: element.as_ref().clone(),
                length: *length,
                stride: u32::try_from(stride).ok()?,
            })
        }
        AbiType::SliceView => {
            let element_kind = source_ty
                .and_then(|ty| {
                    view_element_type_from_type_expr_with_resolver(ty, fallback_resolved, resolver)
                })
                .map(typecheck_slice_element_kind_from_type)
                .unwrap_or(TypecheckSliceElementKind::Other);
            let element_type = source_ty.and_then(|ty| {
                view_element_type_expr_from_type_expr_with_resolver(ty, fallback_resolved, resolver)
            });
            Some(AggregateFieldKind::Slice(SliceTypeInfo {
                element_kind,
                element_type,
            }))
        }
        _ => None,
    }
}

fn view_element_type_expr_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            Some(*view.element.clone())
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            let target_resolved = resolved_for_type_expr(target, resolved, resolver);
            view_element_type_expr_from_type_expr_with_resolver(target, target_resolved, resolver)
        }
        _ => None,
    }
}

fn typecheck_slice_element_kind_from_type(ty: Type) -> TypecheckSliceElementKind {
    match ty {
        Type::I32 => TypecheckSliceElementKind::I32,
        Type::U8 => TypecheckSliceElementKind::U8,
        Type::Usize => TypecheckSliceElementKind::Usize,
        Type::Bool => TypecheckSliceElementKind::Bool,
        Type::Str => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}
