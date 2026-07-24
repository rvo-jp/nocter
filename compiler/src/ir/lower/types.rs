//! Shared type normalization for IR lowering.
//!
//! Converts resolved AST type expressions into the limited IR type set used by v0 lowering.

use crate::abi::{AbiType, AbiValue, ValueClassification, abi_value_from_type_expr};
use crate::ast::{BorrowType, TypeExpr};
use crate::ir::Type;
use crate::resolve::ResolveOutput;
use std::collections::HashSet;

pub(super) fn return_type_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    return_type_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

pub(super) fn return_type_expr_is_top_level_optional(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    return_type_expr_is_top_level_optional_inner(ty, resolved, &mut HashSet::new())
}

fn return_type_expr_is_top_level_optional_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Optional(_) => true,
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result =
                return_type_expr_is_top_level_optional_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}

fn return_type_from_type_expr_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<Type> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => Some(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Some(Type::Never),
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return scalar_or_view_type_from_type_expr(ty, resolved)
                    .or_else(|| aggregate_type_from_type_expr(ty, resolved));
            };
            let Some(target) = &symbol.alias_target else {
                return scalar_or_view_type_from_type_expr(ty, resolved)
                    .or_else(|| aggregate_type_from_type_expr(ty, resolved));
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = return_type_from_type_expr_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => {
            return_type_from_type_expr_inner(&fallible.success, resolved, resolving_names)
                .map(|success| Type::Fallible(Box::new(success)))
        }
        TypeExpr::Optional(optional) => {
            return_type_from_type_expr_inner(&optional.inner, resolved, resolving_names)
                .map(|success| Type::Fallible(Box::new(success)))
        }
        _ => scalar_or_view_type_from_type_expr(ty, resolved)
            .or_else(|| aggregate_type_from_type_expr(ty, resolved)),
    }
}

pub(super) fn parameter_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    parameter_type_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn parameter_type_from_type_expr_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<Type> {
    if let TypeExpr::Reference(reference) = ty
        && let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name)
        && let Some(target) = &symbol.alias_target
    {
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return None;
        }
        let result = parameter_type_from_type_expr_inner(target, resolved, resolving_names);
        resolving_names.remove(&symbol.canonical_name);
        return result;
    }

    if let Some(ty) = scalar_or_view_type_from_type_expr(ty, resolved) {
        return Some(ty);
    }

    if let TypeExpr::Borrow(borrow) = ty {
        return borrow_inner_type(&borrow.inner, resolved).map(|inner| Type::Borrow {
            is_readwrite: borrow.is_readwrite,
            inner: Box::new(inner),
        });
    }

    aggregate_type_from_type_expr(ty, resolved)
}

pub(super) fn scalar_or_view_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    if let Some(ty) = view_type_from_type_expr(ty, resolved) {
        return Some(ty);
    }

    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    match &value.ty {
        AbiType::I32 => Some(Type::I32),
        AbiType::U8 => Some(Type::U8),
        AbiType::Usize => Some(Type::Usize),
        AbiType::Bool => Some(Type::Bool),
        _ => None,
    }
}

pub(super) fn aggregate_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

pub(super) fn aggregate_type_from_abi_value(value: &AbiValue) -> Option<Type> {
    if !matches!(value.ty, AbiType::Struct(_)) {
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

pub(super) fn borrow_inner_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    match &value.ty {
        AbiType::I32 => Some(Type::I32),
        AbiType::U8 => Some(Type::U8),
        AbiType::Usize => Some(Type::Usize),
        AbiType::Bool => Some(Type::Bool),
        AbiType::Struct(_) => aggregate_type_from_abi_value(&value),
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
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
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

fn view_type_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    view_type_from_type_expr_inner(ty, resolved, &mut HashSet::new())
}

fn view_type_from_type_expr_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<Type> {
    match ty {
        TypeExpr::Borrow(borrow) => {
            let value = abi_value_from_type_expr(ty, resolved).ok()?;
            match &value.ty {
                AbiType::StrView if !borrow.is_readwrite => Some(Type::Str),
                AbiType::SliceView if type_expr_is_u8_slice_data_type(&borrow.inner, resolved) => {
                    Some(Type::Slice {
                        is_readwrite: borrow.is_readwrite,
                    })
                }
                _ => None,
            }
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            let Some(target) = &symbol.alias_target else {
                return None;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = view_type_from_type_expr_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

fn type_expr_is_u8_slice_data_type(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_is_u8_slice_data_type_inner(ty, resolved, &mut HashSet::new())
}

fn type_expr_is_u8_slice_data_type_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::View(view) => {
            !view.is_readwrite
                && matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
        }
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return false;
            };
            let Some(target) = &symbol.alias_target else {
                return false;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = type_expr_is_u8_slice_data_type_inner(target, resolved, resolving_names);
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => false,
    }
}
