use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::{Expr, TypeExpr};
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use std::collections::HashSet;

pub(super) fn type_expr_is_copy(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<bool> {
    type_expr_is_copy_inner(ty, resolved, &mut HashSet::new())
}

pub(super) fn implicit_non_copy_struct_identifier_source<'a>(
    expression: &'a Expr,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<(&'a str, String)> {
    match expression {
        Expr::Identifier(identifier) => {
            let value_type = expression_type(expression, resolved, environment);
            non_copy_struct_type_display(&value_type, resolved)
                .map(|type_name| (identifier.name.as_str(), type_name))
        }
        Expr::Group(group) => {
            implicit_non_copy_struct_identifier_source(&group.expression, resolved, environment)
        }
        _ => None,
    }
}

pub(super) fn non_copy_struct_type_name<'a>(
    ty: &Type,
    resolved: &'a ResolveOutput,
) -> Option<&'a str> {
    non_copy_struct_symbol(ty, resolved).map(|symbol| symbol.canonical_name.as_str())
}

fn non_copy_struct_type_display(ty: &Type, resolved: &ResolveOutput) -> Option<String> {
    non_copy_struct_symbol(ty, resolved).map(|_| ty.display())
}

fn non_copy_struct_symbol<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;
    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && !symbol.is_copy)
}

fn type_expr_is_copy_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<bool> {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
            | "isize" => Some(true),
            "str" | "error" | "void" | "never" | "Self" => Some(false),
            name => resolved
                .type_symbol_by_reference_name(name)
                .map(|symbol| type_symbol_is_copy(symbol, resolved, resolving_names)),
        },
        TypeExpr::Borrow(borrow) => Some(!borrow.is_readwrite),
        TypeExpr::Pointer(_) => Some(true),
        TypeExpr::Array(array) => {
            type_expr_is_copy_inner(&array.element, resolved, resolving_names)
        }
        TypeExpr::Optional(optional) => {
            type_expr_is_copy_inner(&optional.inner, resolved, resolving_names)
        }
        TypeExpr::Fallible(fallible) => {
            let success = type_expr_is_copy_inner(&fallible.success, resolved, resolving_names)?;
            let error = type_expr_is_copy_inner(&fallible.error, resolved, resolving_names)?;
            Some(success && error)
        }
        TypeExpr::Generic(_) | TypeExpr::View(_) => None,
    }
}

fn type_is_copy_inner(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::I32 | Type::Primitive(_) | Type::Str | Type::Pointer(_) => true,
        Type::View {
            is_readwrite: false,
            ..
        } => true,
        Type::Array { element, .. } | Type::Optional(element) => {
            type_is_copy_inner(element, resolved, resolving_names)
        }
        Type::Fallible { success, error } => {
            type_is_copy_inner(success, resolved, resolving_names)
                && type_is_copy_inner(error, resolved, resolving_names)
        }
        Type::Named(name) | Type::Generic { name, .. } => resolved
            .type_symbol_by_canonical_name(name)
            .is_some_and(|symbol| type_symbol_is_copy(symbol, resolved, resolving_names)),
        Type::StrData
        | Type::Error
        | Type::Void
        | Type::Never
        | Type::None
        | Type::ArrayData { .. }
        | Type::View {
            is_readwrite: true, ..
        }
        | Type::Parameter(_)
        | Type::Unresolved(_)
        | Type::Unknown => false,
    }
}

fn type_symbol_is_copy(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }

    let is_copy = match symbol.kind {
        TypeSymbolKind::Struct => symbol.is_copy,
        TypeSymbolKind::Enum => symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty()),
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
            type_expr_is_copy_inner(target, resolved, resolving_names).unwrap_or_else(|| {
                let target_type = super::type_expr::type_expr_to_type(target, resolved);
                type_is_copy_inner(&target_type, resolved, resolving_names)
            })
        }),
        TypeSymbolKind::Interface => false,
    };

    resolving_names.remove(&symbol.canonical_name);
    is_copy
}
