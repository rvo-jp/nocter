use super::field_layouts::{
    generic_type_expr_substitutions, resolved_for_type_expr, type_symbol_by_reference_name,
};
use super::*;

pub(in crate::ir::lower) fn supported_aggregate_copy_layout(layout: ValueLayout) -> bool {
    layout.size > 0
}

pub(in crate::ir::lower) fn aggregate_type_layout(ty: &Type) -> Option<ValueLayout> {
    match ty {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => Some(*layout),
        _ => None,
    }
}

pub(in crate::ir::lower) fn aggregate_call_return_layout_from_resolved(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ValueLayout> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    let value = abi_value_from_type_expr_with_resolver(&return_type, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    if matches!(value.ty, AbiType::Struct(_) | AbiType::Array { .. }) {
        Some(value.layout)
    } else {
        None
    }
}

pub(in crate::ir::lower) fn type_expr_is_copy_struct(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    type_expr_is_copy_struct_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(in crate::ir::lower) fn type_expr_is_copy_struct_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_copy_struct_inner(ty, fallback_resolved, &resolver, &mut HashSet::new())
}

pub(in crate::ir::lower) fn type_expr_is_copy_aggregate_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if type_expr_is_copy_fixed_array_value_with_resolver(ty, fallback_resolved, &resolver) {
        return true;
    }
    type_expr_is_copy_struct_with_resolver(ty, fallback_resolved, resolver)
}

fn type_expr_is_copy_fixed_array_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Fallible(fallible) => {
            return type_expr_is_copy_fixed_array_value_with_resolver(
                &fallible.success,
                fallback_resolved,
                resolver,
            );
        }
        TypeExpr::Optional(optional) => {
            return type_expr_is_copy_fixed_array_value_with_resolver(
                &optional.inner,
                fallback_resolved,
                resolver,
            );
        }
        _ => {}
    }

    abi_value_from_type_expr(ty, fallback_resolved).is_ok_and(fixed_array_value_is_runtime_copy)
        || abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver)
            .is_ok_and(fixed_array_value_is_runtime_copy)
}

pub(super) fn fixed_array_element_abi_is_runtime_copy(element: &AbiType) -> bool {
    matches!(
        element,
        AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::StrView
    )
}

fn fixed_array_value_is_runtime_copy(value: AbiValue) -> bool {
    matches!(
        value.ty,
        AbiType::Array { ref element, .. } if fixed_array_element_abi_is_runtime_copy(element)
    )
}

fn type_expr_is_copy_struct_inner<'a, F>(
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
            type_symbol_is_copy_struct_inner(
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
            let Some(substitutions) = generic_type_expr_substitutions(symbol, ty) else {
                return false;
            };
            type_symbol_is_copy_struct_inner(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Fallible(fallible) => type_expr_is_copy_struct_inner(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_copy_struct_inner(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        _ => false,
    }
}

fn type_symbol_is_copy_struct_inner<'a, F>(
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
        TypeSymbolKind::Struct => copy_struct_fields_are_copy_values(
            symbol,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        ),
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
            let target = substitute_type_expr_parameters(target, substitutions);
            type_expr_is_copy_struct_inner(&target, fallback_resolved, resolver, resolving_names)
        }),
        TypeSymbolKind::Enum | TypeSymbolKind::Interface => false,
    };

    resolving_names.remove(&symbol.canonical_name);
    is_copy
}

fn copy_struct_fields_are_copy_values<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    symbol.fields.iter().all(|field| {
        let field_ty = substitute_type_expr_parameters(&field.ty, substitutions);
        type_expr_is_copy_value_inner(&field_ty, fallback_resolved, resolver, resolving_names)
    })
}

fn type_expr_is_copy_value_inner<'a, F>(
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
                type_symbol_is_copy_value_inner(
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
            let Some(substitutions) = generic_type_expr_substitutions(symbol, ty) else {
                return false;
            };
            type_symbol_is_copy_value_inner(
                symbol,
                fallback_resolved,
                resolver,
                &substitutions,
                resolving_names,
            )
        }
        TypeExpr::Borrow(borrow) => !borrow.is_readwrite,
        TypeExpr::Pointer(_) => true,
        TypeExpr::Array(array) => type_expr_is_copy_value_inner(
            &array.element,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => type_expr_is_copy_value_inner(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Fallible(fallible) => {
            type_expr_is_copy_value_inner(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            ) && type_expr_is_copy_value_inner(
                &fallible.error,
                fallback_resolved,
                resolver,
                resolving_names,
            )
        }
        TypeExpr::View(_) => false,
    }
}

fn type_symbol_is_copy_value_inner<'a, F>(
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
        TypeSymbolKind::Struct => type_symbol_is_copy_struct_inner(
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
                type_expr_is_copy_value_inner(&target, fallback_resolved, resolver, resolving_names)
            });
            resolving_names.remove(&symbol.canonical_name);
            is_copy
        }
        TypeSymbolKind::Interface => false,
    }
}
