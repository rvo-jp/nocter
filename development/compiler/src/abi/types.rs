use super::layout::{align_to, layout_of};
use super::{
    AbiEnum, AbiEnumVariant, AbiField, AbiReturn, AbiType, AbiTypeError, AbiValue, ValueLayout,
};
use crate::ast::{TypeExpr, canonical_type_expr, substitute_type_expr_parameters};
use crate::literals::decode_integer_literal_value;
use crate::outcomes::{OutcomeLayer, storage::outcome_storage_layout};
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::SourceId;
use std::collections::{HashMap, HashSet};

pub fn abi_value_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<AbiValue, AbiTypeError> {
    abi_value_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub fn abi_value_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<AbiValue, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_inner(ty, fallback_resolved, &resolver)
}

pub fn abi_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<AbiType, AbiTypeError> {
    abi_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub fn abi_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<AbiType, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match abi_type_kind_from_type_expr(
        ty,
        fallback_resolved,
        &resolver,
        &HashMap::new(),
        &mut HashSet::new(),
    )? {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
        AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(canonical_type_expr(ty))),
    }
}

pub(in crate::abi) fn abi_value_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiValue, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    AbiValue::from_abi_type(abi_type_from_type_expr_with_resolver_inner(
        ty,
        fallback_resolved,
        resolver,
    )?)
}

fn abi_type_from_type_expr_with_resolver_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiType, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match abi_type_kind_from_type_expr(
        ty,
        fallback_resolved,
        resolver,
        &HashMap::new(),
        &mut HashSet::new(),
    )? {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
        AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(canonical_type_expr(ty))),
    }
}

pub(in crate::abi) fn abi_return_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiReturn, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_return_from_type_expr_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(in crate::abi) fn abi_return_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiReturn, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(AbiReturn::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Ok(AbiReturn::Never),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return abi_value_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .map(AbiReturn::Value);
            };
            let Some(target) = &symbol.alias_target else {
                return abi_value_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .map(AbiReturn::Value);
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
            }
            let result = abi_return_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => abi_value_from_type_expr_inner(ty, fallback_resolved, resolver).map(AbiReturn::Value),
    }
}

enum AbiTypeKind {
    Value(AbiType),
    UnsizedStr,
    UnsizedArray,
}

pub(in crate::abi) fn resolved_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> &'a ResolveOutput
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    resolver(ty.span().source).unwrap_or(fallback_resolved)
}

pub(in crate::abi) fn type_symbol_by_reference_name<'a>(
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

fn abi_type_kind_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Callable(_) => Err(AbiTypeError::UnsupportedType(canonical_type_expr(ty))),
        TypeExpr::Closure(closure) => {
            let fields = closure
                .captures
                .iter()
                .map(|capture| {
                    let ty = sized_abi_type_kind(
                        abi_type_kind_from_type_expr(
                            &capture.ty,
                            fallback_resolved,
                            resolver,
                            substitutions,
                            resolving_names,
                        )?,
                        &capture.ty,
                    )?;
                    Ok(AbiField::new(capture.name.clone(), ty))
                })
                .collect::<Result<Vec<_>, AbiTypeError>>()?;
            Ok(AbiTypeKind::Value(AbiType::Struct(fields)))
        }
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" => Ok(AbiTypeKind::Value(AbiType::Bool)),
            "u8" => Ok(AbiTypeKind::Value(AbiType::U8)),
            "i8" => Ok(AbiTypeKind::Value(AbiType::I8)),
            "u16" => Ok(AbiTypeKind::Value(AbiType::U16)),
            "i16" => Ok(AbiTypeKind::Value(AbiType::I16)),
            "u32" => Ok(AbiTypeKind::Value(AbiType::U32)),
            "i32" => Ok(AbiTypeKind::Value(AbiType::I32)),
            "u64" => Ok(AbiTypeKind::Value(AbiType::U64)),
            "i64" => Ok(AbiTypeKind::Value(AbiType::I64)),
            "usize" => Ok(AbiTypeKind::Value(AbiType::Usize)),
            "isize" => Ok(AbiTypeKind::Value(AbiType::Isize)),
            "str" => Ok(AbiTypeKind::UnsizedStr),
            "void" | "never" | "error" => {
                Err(AbiTypeError::UnsupportedType(reference.name.clone()))
            }
            name if substitutions.contains_key(name) => {
                let Some(substitution) = substitutions.get(name) else {
                    return Err(AbiTypeError::UnresolvedType(name.to_string()));
                };
                let substitution = substitute_type_expr_parameters(substitution, substitutions);
                abi_type_kind_from_type_expr(
                    &substitution,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            }
            name => {
                let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
                let Some(symbol) = type_symbol_by_reference_name(resolved, name) else {
                    return Err(AbiTypeError::UnresolvedType(name.to_string()));
                };
                if symbol.generic_arity > 0 {
                    return Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()));
                }
                abi_type_kind_from_symbol(
                    symbol,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            }
        },
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &generic.name) else {
                return Err(AbiTypeError::UnresolvedType(generic.name.clone()));
            };
            if symbol.generic_arity != generic.arguments.len() {
                return Err(AbiTypeError::UnsupportedType(canonical_type_expr(ty)));
            }

            let mut instantiated_substitutions = substitutions.clone();
            for (parameter, argument) in symbol
                .generic_parameters
                .iter()
                .zip(generic.arguments.iter())
            {
                instantiated_substitutions.insert(
                    parameter.clone(),
                    substitute_type_expr_parameters(argument, substitutions),
                );
            }
            abi_type_kind_from_symbol(
                symbol,
                fallback_resolved,
                resolver,
                &instantiated_substitutions,
                resolving_names,
            )
        }
        TypeExpr::Pointer(_) => Ok(AbiTypeKind::Value(AbiType::Pointer)),
        TypeExpr::Borrow(borrow) => {
            match abi_type_kind_from_type_expr(
                &borrow.inner,
                fallback_resolved,
                resolver,
                substitutions,
                resolving_names,
            )? {
                AbiTypeKind::UnsizedStr => Ok(AbiTypeKind::Value(AbiType::StrView)),
                AbiTypeKind::UnsizedArray => Ok(AbiTypeKind::Value(AbiType::SliceView)),
                AbiTypeKind::Value(_) => Ok(AbiTypeKind::Value(AbiType::Borrow)),
            }
        }
        TypeExpr::View(_) => Ok(AbiTypeKind::UnsizedArray),
        TypeExpr::Array(array) => {
            let element = match abi_type_kind_from_type_expr(
                &array.element,
                fallback_resolved,
                resolver,
                substitutions,
                resolving_names,
            )? {
                AbiTypeKind::Value(ty) => ty,
                AbiTypeKind::UnsizedStr => {
                    return Err(AbiTypeError::UnsizedValue("str".to_string()));
                }
                AbiTypeKind::UnsizedArray => {
                    return Err(AbiTypeError::UnsizedValue(canonical_type_expr(
                        &array.element,
                    )));
                }
            };
            let Some(length) = decode_integer_literal_value(&array.length.value)
                .and_then(|value| u64::try_from(value).ok())
            else {
                return Err(AbiTypeError::UnsupportedType(format!(
                    "[{}; {}]",
                    canonical_type_expr(&array.element),
                    array.length.value
                )));
            };
            Ok(AbiTypeKind::Value(AbiType::Array {
                element: Box::new(element),
                length,
            }))
        }
        TypeExpr::Optional(optional) => {
            let payload = sized_abi_type_kind(
                abi_type_kind_from_type_expr(
                    &optional.inner,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )?,
                &optional.inner,
            )?;
            let payload_layout = layout_of(&payload)?;
            let storage = outcome_storage_layout(&[OutcomeLayer::Optional], payload_layout);
            Ok(AbiTypeKind::Value(AbiType::Outcome {
                layout: storage.layout,
            }))
        }
        TypeExpr::Fallible(fallible) => {
            let payload = sized_abi_type_kind(
                abi_type_kind_from_type_expr(
                    &fallible.success,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )?,
                &fallible.success,
            )?;
            let payload_layout = layout_of(&payload)?;
            let storage = outcome_storage_layout(&[OutcomeLayer::Fallible], payload_layout);
            Ok(AbiTypeKind::Value(AbiType::Outcome {
                layout: storage.layout,
            }))
        }
    }
}

fn sized_abi_type_kind(kind: AbiTypeKind, source: &TypeExpr) -> Result<AbiType, AbiTypeError> {
    match kind {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr | AbiTypeKind::UnsizedArray => {
            Err(AbiTypeError::UnsizedValue(canonical_type_expr(source)))
        }
    }
}

fn abi_type_kind_from_symbol<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
    }

    let result = (|| match symbol.kind {
        TypeSymbolKind::Alias => {
            if let Some(target) = &symbol.alias_target {
                abi_type_kind_from_type_expr(
                    target,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            } else {
                Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
            }
        }
        TypeSymbolKind::Struct => {
            let mut fields = Vec::with_capacity(symbol.fields.len());
            for field in &symbol.fields {
                let ty = match abi_type_kind_from_type_expr(
                    &field.ty,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )? {
                    AbiTypeKind::Value(ty) => ty,
                    AbiTypeKind::UnsizedStr => {
                        return Err(AbiTypeError::UnsizedValue("str".to_string()));
                    }
                    AbiTypeKind::UnsizedArray => {
                        return Err(AbiTypeError::UnsizedValue(canonical_type_expr(&field.ty)));
                    }
                };
                fields.push(AbiField::new(field.name.clone(), ty));
            }
            Ok(AbiTypeKind::Value(AbiType::Struct(fields)))
        }
        TypeSymbolKind::Enum => enum_abi_type(
            symbol,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        ),
        TypeSymbolKind::Interface => {
            Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
        }
    })();

    resolving_names.remove(&symbol.canonical_name);
    result
}

fn enum_abi_type<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if symbol.variants.len() > u8::MAX as usize + 1 {
        return Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()));
    }

    if symbol
        .variants
        .iter()
        .all(|variant| variant.payload.is_empty())
    {
        return Ok(AbiTypeKind::Value(AbiType::U8));
    }

    let mut variants = Vec::with_capacity(symbol.variants.len());
    let mut payload_size = 0_u64;
    let mut payload_align = 1_u64;

    for (tag, variant) in symbol.variants.iter().enumerate() {
        let payload = enum_variant_payload_abi_type(
            &variant.payload,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        )?;
        if let Some(payload) = &payload {
            let layout = layout_of(payload)?;
            payload_size = payload_size.max(layout.size);
            payload_align = payload_align.max(layout.align);
        }
        variants.push(AbiEnumVariant::new(
            variant.name.clone(),
            u8::try_from(tag)
                .map_err(|_| AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))?,
            payload,
        ));
    }

    Ok(AbiTypeKind::Value(AbiType::Enum(AbiEnum {
        variants,
        payload_offset: align_to(1, payload_align)?,
        payload_layout: ValueLayout::new(payload_size, payload_align),
    })))
}

fn enum_variant_payload_abi_type<'a, F>(
    payload: &[crate::resolve::ParameterSignature],
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<Option<AbiType>, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match payload {
        [] => Ok(None),
        [parameter] => match abi_type_kind_from_type_expr(
            &parameter.ty,
            fallback_resolved,
            resolver,
            substitutions,
            resolving_names,
        )? {
            AbiTypeKind::Value(ty) => Ok(Some(ty)),
            AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
            AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(canonical_type_expr(
                &parameter.ty,
            ))),
        },
        parameters => {
            let mut fields = Vec::with_capacity(parameters.len());
            for parameter in parameters {
                let ty = match abi_type_kind_from_type_expr(
                    &parameter.ty,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )? {
                    AbiTypeKind::Value(ty) => ty,
                    AbiTypeKind::UnsizedStr => {
                        return Err(AbiTypeError::UnsizedValue("str".to_string()));
                    }
                    AbiTypeKind::UnsizedArray => {
                        return Err(AbiTypeError::UnsizedValue(canonical_type_expr(
                            &parameter.ty,
                        )));
                    }
                };
                fields.push(AbiField::new(parameter.name.clone(), ty));
            }
            Ok(Some(AbiType::Struct(fields)))
        }
    }
}
