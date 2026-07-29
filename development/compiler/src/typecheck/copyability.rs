use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::{Expr, TypeExpr};
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NonCopyOwnedValueKind {
    Struct,
    Enum,
    FixedArray,
    Optional,
    Fallible,
}

impl NonCopyOwnedValueKind {
    pub(super) fn noun(self) -> &'static str {
        match self {
            NonCopyOwnedValueKind::Struct => "non-copy struct",
            NonCopyOwnedValueKind::Enum => "move-only enum",
            NonCopyOwnedValueKind::FixedArray => "move-only fixed array",
            NonCopyOwnedValueKind::Optional => "move-only optional value",
            NonCopyOwnedValueKind::Fallible => "move-only fallible value",
        }
    }

    pub(super) fn copy_help(self, source_name: &str, type_name: &str) -> String {
        match self {
            NonCopyOwnedValueKind::Struct => format!(
                "declare `{type_name}` with `copy struct` or write `move {source_name}` to transfer ownership"
            ),
            NonCopyOwnedValueKind::Enum => format!(
                "payload-carrying enums are move-only in v0; write `move {source_name}` to transfer ownership"
            ),
            NonCopyOwnedValueKind::FixedArray => format!(
                "make the fixed array element type copyable or write `move {source_name}` to transfer ownership"
            ),
            NonCopyOwnedValueKind::Optional => format!(
                "make the optional payload type copyable or write `move {source_name}` to transfer ownership"
            ),
            NonCopyOwnedValueKind::Fallible => format!(
                "make the fallible payload types copyable or write `move {source_name}` to transfer ownership"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NonCopyOwnedValueSource {
    pub(super) source_name: String,
    pub(super) type_name: String,
    pub(super) kind: NonCopyOwnedValueKind,
}

pub(super) fn type_expr_is_copy(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<bool> {
    type_expr_is_copy_inner(ty, resolved, &mut HashSet::new())
}

pub(super) fn implicit_non_copy_owned_value_source(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<NonCopyOwnedValueSource> {
    match expression {
        Expr::Identifier(identifier) => {
            let value_type = expression_type(expression, resolved, environment);
            let (kind, type_name) = non_copy_owned_type_kind_and_display(&value_type, resolved)?;
            Some(NonCopyOwnedValueSource {
                source_name: identifier.name.clone(),
                type_name,
                kind,
            })
        }
        Expr::Member(member) if !member_expression_is_enum_variant(member, resolved) => {
            let value_type = expression_type(expression, resolved, environment);
            let (kind, type_name) = non_copy_owned_type_kind_and_display(&value_type, resolved)?;
            member_source_path(expression).map(|source_name| NonCopyOwnedValueSource {
                source_name,
                type_name,
                kind,
            })
        }
        Expr::Group(group) => {
            implicit_non_copy_owned_value_source(&group.expression, resolved, environment)
        }
        _ => None,
    }
}

fn member_expression_is_enum_variant(
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
) -> bool {
    let Expr::Identifier(enum_name) = member.object.as_ref() else {
        return false;
    };

    resolved
        .type_symbol_by_name(&enum_name.name)
        .is_some_and(|symbol| symbol.kind == TypeSymbolKind::Enum)
}

fn member_source_path(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier.name.clone()),
        Expr::Member(member) => {
            let object = member_source_path(&member.object)?;
            Some(format!("{object}.{}", member.member))
        }
        Expr::Group(group) => member_source_path(&group.expression),
        _ => None,
    }
}

pub(super) fn non_copy_struct_type_name<'a>(
    ty: &Type,
    resolved: &'a ResolveOutput,
) -> Option<&'a str> {
    non_copy_struct_symbol(ty, resolved).map(|symbol| symbol.canonical_name.as_str())
}

pub(super) fn non_copy_owned_type_kind(
    ty: &Type,
    resolved: &ResolveOutput,
) -> Option<NonCopyOwnedValueKind> {
    non_copy_owned_type_kind_and_display(ty, resolved).map(|(kind, _)| kind)
}

fn non_copy_owned_type_kind_and_display(
    ty: &Type,
    resolved: &ResolveOutput,
) -> Option<(NonCopyOwnedValueKind, String)> {
    non_copy_owned_type_kind_inner(ty, resolved, &mut HashSet::new())
        .map(|kind| (kind, ty.display()))
}

fn non_copy_owned_type_kind_inner(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<NonCopyOwnedValueKind> {
    match ty {
        Type::Named(name) | Type::Generic { name, .. } => {
            let symbol = resolved.type_symbol_by_canonical_name(name)?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }

            let kind = match symbol.kind {
                TypeSymbolKind::Struct if !symbol.is_copy => Some(NonCopyOwnedValueKind::Struct),
                TypeSymbolKind::Enum
                    if symbol
                        .variants
                        .iter()
                        .any(|variant| !variant.payload.is_empty()) =>
                {
                    Some(NonCopyOwnedValueKind::Enum)
                }
                TypeSymbolKind::Alias => symbol.alias_target.as_ref().and_then(|target| {
                    let target_type = super::type_expr::type_expr_to_type(target, resolved);
                    non_copy_owned_type_kind_inner(&target_type, resolved, resolving_names)
                }),
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            };

            resolving_names.remove(&symbol.canonical_name);
            kind
        }
        Type::Array { element, .. }
            if !type_is_copy_inner(element, resolved, &mut HashSet::new()) =>
        {
            Some(NonCopyOwnedValueKind::FixedArray)
        }
        Type::Optional(inner) if !type_is_copy_inner(inner, resolved, &mut HashSet::new()) => {
            Some(NonCopyOwnedValueKind::Optional)
        }
        Type::Fallible { success, error }
            if !type_is_copy_inner(success, resolved, &mut HashSet::new())
                || !type_is_copy_inner(error, resolved, &mut HashSet::new()) =>
        {
            Some(NonCopyOwnedValueKind::Fallible)
        }
        _ => None,
    }
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
            | "isize" | "error" => Some(true),
            "str" | "void" | "never" | "Self" => Some(false),
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
        Type::I32 | Type::Primitive(_) | Type::Str | Type::Error | Type::Pointer(_) => true,
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
