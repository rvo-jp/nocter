//! Shared type normalization for IR lowering.
//!
//! Converts resolved AST type expressions into the native lowerer's IR type set.

use crate::abi::{AbiType, AbiValue, ValueClassification, abi_value_from_type_expr_with_resolver};
use crate::ast::{
    ArrayType, BorrowType, FallibleType, GenericType, OptionalType, PointerType, TypeExpr, ViewType,
};
use crate::ir::Type;
use crate::outcomes::{OutcomeLayer, outcome_shape_with_resolver};
use crate::resolve::{ResolveOutput, TypeSymbol};
use crate::source::SourceId;
use std::collections::HashSet;

pub(super) fn return_type_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    return_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(super) fn type_expr_with_self_type(ty: &TypeExpr, self_ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::Callable(callable) => {
            let mut callable = callable.clone();
            for parameter in &mut callable.parameters {
                parameter.ty = type_expr_with_self_type(&parameter.ty, self_ty);
            }
            callable.return_type =
                Box::new(type_expr_with_self_type(&callable.return_type, self_ty));
            TypeExpr::Callable(callable)
        }
        TypeExpr::Closure(_) => ty.clone(),
        TypeExpr::Reference(reference) if reference.name == "Self" => self_ty.clone(),
        TypeExpr::Reference(_) => ty.clone(),
        TypeExpr::Generic(generic) => TypeExpr::Generic(GenericType {
            span: generic.span,
            name: generic.name.clone(),
            name_span: generic.name_span,
            arguments: generic
                .arguments
                .iter()
                .map(|argument| type_expr_with_self_type(argument, self_ty))
                .collect(),
        }),
        TypeExpr::Projection(projection) => TypeExpr::Projection(crate::ast::ProjectedType {
            span: projection.span,
            base: Box::new(type_expr_with_self_type(&projection.base, self_ty)),
            name: projection.name.clone(),
            name_span: projection.name_span,
        }),
        TypeExpr::Pointer(pointer) => TypeExpr::Pointer(PointerType {
            span: pointer.span,
            inner: Box::new(type_expr_with_self_type(&pointer.inner, self_ty)),
        }),
        TypeExpr::Borrow(borrow) => TypeExpr::Borrow(BorrowType {
            span: borrow.span,
            is_readwrite: borrow.is_readwrite,
            inner: Box::new(type_expr_with_self_type(&borrow.inner, self_ty)),
        }),
        TypeExpr::View(view) => TypeExpr::View(ViewType {
            span: view.span,
            is_readwrite: view.is_readwrite,
            element: Box::new(type_expr_with_self_type(&view.element, self_ty)),
        }),
        TypeExpr::Array(array) => TypeExpr::Array(ArrayType {
            span: array.span,
            element: Box::new(type_expr_with_self_type(&array.element, self_ty)),
            length: array.length.clone(),
        }),
        TypeExpr::Optional(optional) => TypeExpr::Optional(OptionalType {
            span: optional.span,
            inner: Box::new(type_expr_with_self_type(&optional.inner, self_ty)),
        }),
        TypeExpr::Fallible(fallible) => TypeExpr::Fallible(FallibleType {
            span: fallible.span,
            success: Box::new(type_expr_with_self_type(&fallible.success, self_ty)),
            error: Box::new(type_expr_with_self_type(&fallible.error, self_ty)),
        }),
    }
}

pub(super) fn return_type_expr_is_top_level_optional(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    return_type_expr_is_top_level_optional_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(super) fn return_type_expr_has_optional_layer(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    return_type_expr_has_optional_layer_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(super) fn return_type_expr_has_optional_layer_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    outcome_shape_with_resolver(ty, fallback_resolved, resolver)
        .layers
        .contains(&OutcomeLayer::Optional)
}

pub(super) fn return_type_expr_is_top_level_optional_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    return_type_expr_is_top_level_optional_inner(
        ty,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
    )
}

pub(super) fn top_level_optional_success_abi_value_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<AbiValue>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    top_level_optional_success_abi_value_inner(
        ty,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
    )
}

fn top_level_optional_success_abi_value_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<AbiValue>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(optional) => {
            abi_value_from_type_expr_with_resolver(&optional.inner, fallback_resolved, resolver)
                .ok()
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = top_level_optional_success_abi_value_inner(
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

fn return_type_expr_is_top_level_optional_inner<'a, F>(
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
            let result = return_type_expr_is_top_level_optional_inner(
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

pub(super) fn return_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let shape = outcome_shape_with_resolver(ty, fallback_resolved, &resolver);
    if let [outer, inner] = shape.layers.as_slice() {
        let payload = return_type_from_type_expr_inner(
            &shape.payload,
            fallback_resolved,
            &resolver,
            &mut HashSet::new(),
        )?;
        return Some(Type::ComposedOutcome {
            outer: *outer,
            inner: *inner,
            payload: Box::new(payload),
        });
    }
    return_type_from_type_expr_inner(ty, fallback_resolved, &resolver, &mut HashSet::new())
}

fn return_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => Some(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Some(Type::Never),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .or_else(|| {
                        aggregate_type_from_type_expr_inner(ty, fallback_resolved, resolver)
                    });
            };
            let Some(target) = &symbol.alias_target else {
                return scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .or_else(|| {
                        aggregate_type_from_type_expr_inner(ty, fallback_resolved, resolver)
                    });
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = return_type_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => return_type_from_type_expr_inner(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        )
        .map(|success| Type::Fallible(Box::new(success))),
        TypeExpr::Optional(optional) => return_type_from_type_expr_inner(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        )
        .map(|payload| Type::Optional(Box::new(payload))),
        TypeExpr::Borrow(borrow) => {
            scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, resolver).or_else(
                || {
                    borrow_inner_type_inner(&borrow.inner, fallback_resolved, resolver).map(
                        |inner| Type::Borrow {
                            is_readwrite: borrow.is_readwrite,
                            inner: Box::new(inner),
                        },
                    )
                },
            )
        }
        _ => scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, resolver)
            .or_else(|| aggregate_type_from_type_expr_inner(ty, fallback_resolved, resolver)),
    }
}

pub(super) fn parameter_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    parameter_type_from_type_expr_inner(ty, fallback_resolved, &resolver, &mut HashSet::new())
}

fn parameter_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let TypeExpr::Reference(reference) = ty {
        let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
        if let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name)
            && let Some(target) = &symbol.alias_target
        {
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = parameter_type_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            return result;
        }
    }

    if let TypeExpr::Reference(reference) = ty
        && reference.name == "error"
    {
        return Some(Type::Error);
    }

    let shape = outcome_shape_with_resolver(ty, fallback_resolved, resolver);
    if !shape.layers.is_empty() {
        let payload = parameter_type_from_type_expr_inner(
            &shape.payload,
            fallback_resolved,
            resolver,
            resolving_names,
        )?;
        return match shape.layers.as_slice() {
            [OutcomeLayer::Optional] => Some(Type::Optional(Box::new(payload))),
            [OutcomeLayer::Fallible] => Some(Type::Fallible(Box::new(payload))),
            [outer, inner] => Some(Type::ComposedOutcome {
                outer: *outer,
                inner: *inner,
                payload: Box::new(payload),
            }),
            _ => None,
        };
    }

    if let Some(ty) = scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, resolver) {
        return Some(ty);
    }

    if let TypeExpr::Borrow(borrow) = ty {
        return borrow_inner_type_inner(&borrow.inner, fallback_resolved, resolver).map(|inner| {
            Type::Borrow {
                is_readwrite: borrow.is_readwrite,
                inner: Box::new(inner),
            }
        });
    }

    aggregate_type_from_type_expr_inner(ty, fallback_resolved, resolver)
}

pub(super) fn scalar_or_view_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    scalar_or_view_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(super) fn scalar_or_view_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    scalar_or_view_type_from_type_expr_inner(ty, fallback_resolved, &resolver)
}

fn scalar_or_view_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let Some(ty) =
        view_type_from_type_expr_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
    {
        return Some(ty);
    }

    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    match &value.ty {
        AbiType::I32 => Some(Type::I32),
        AbiType::U8 => Some(Type::U8),
        AbiType::Usize | AbiType::Pointer => Some(Type::Usize),
        ty if ty.integer_type().is_some() => Some(Type::Integer(ty.integer_type()?)),
        AbiType::Bool => Some(Type::Bool),
        _ => None,
    }
}

pub(super) fn view_element_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    view_element_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub(super) fn view_element_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    view_element_type_from_type_expr_inner(ty, fallback_resolved, &resolver, &mut HashSet::new())
}

fn view_element_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            scalar_or_view_type_from_type_expr_inner(&view.element, fallback_resolved, resolver)
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let Some(target) = &symbol.alias_target else {
                return None;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = view_element_type_from_type_expr_inner(
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

fn aggregate_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    aggregate_type_from_abi_value(&value)
}

pub(super) fn aggregate_type_from_abi_value(value: &AbiValue) -> Option<Type> {
    if !matches!(
        value.ty,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) | AbiType::Outcome { .. }
    ) {
        return None;
    }

    match value.classification {
        ValueClassification::Indirect => Some(Type::Aggregate {
            layout: value.layout,
        }),
        ValueClassification::Direct { words } => Some(Type::DirectAggregate {
            layout: value.layout,
            words,
        }),
    }
}

pub(super) fn borrow_inner_type_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    borrow_inner_type_inner(ty, fallback_resolved, &resolver)
}

fn borrow_inner_type_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).ok()?;
    match &value.ty {
        AbiType::I32 => Some(Type::I32),
        AbiType::U8 => Some(Type::U8),
        AbiType::Usize | AbiType::Pointer => Some(Type::Usize),
        ty if ty.integer_type().is_some() => Some(Type::Integer(ty.integer_type()?)),
        AbiType::Bool => Some(Type::Bool),
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) | AbiType::Outcome { .. } => {
            aggregate_type_from_abi_value(&value)
        }
        _ => None,
    }
}

pub(super) fn borrow_type_from_type_expr<'a>(
    ty: &'a TypeExpr,
    resolved: &'a ResolveOutput,
) -> Option<&'a BorrowType> {
    borrow_type_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn borrow_type_from_type_expr_inner<'a>(
    ty: &'a TypeExpr,
    resolved: &'a ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<&'a BorrowType> {
    match ty {
        TypeExpr::Borrow(borrow) => Some(borrow),
        TypeExpr::Reference(reference) => {
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let Some(target) = &symbol.alias_target else {
                return None;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = borrow_type_from_type_expr_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn view_type_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<Type>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            let value = abi_value_from_type_expr_with_resolver(ty, fallback_resolved, |source| {
                resolver(source)
            })
            .ok()?;
            match &value.ty {
                AbiType::StrView if !borrow.is_readwrite => Some(Type::Str),
                AbiType::SliceView => Some(Type::Slice {
                    is_readwrite: borrow.is_readwrite,
                }),
                _ => None,
            }
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let Some(target) = &symbol.alias_target else {
                return None;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = view_type_from_type_expr_inner(
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

fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}
