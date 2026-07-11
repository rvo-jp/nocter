use super::model::{Type, TypeEnvironment};
use crate::ast::TypeExpr;
use crate::resolve::ResolveOutput;
use std::collections::HashSet;

pub(super) fn type_expr_to_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Type {
    type_expr_to_type_with_self_type(ty, resolved, None)
}

pub(super) fn type_expr_to_type_in_environment(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    type_expr_to_type_with_self_type(ty, resolved, environment.self_type())
}

pub(super) fn type_expr_to_type_with_self_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Type {
    type_expr_to_type_inner(ty, resolved, self_type, &mut HashSet::new())
}

fn type_expr_to_type_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    resolving_aliases: &mut HashSet<String>,
) -> Type {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "Self" => self_type
                .cloned()
                .unwrap_or_else(|| Type::Unresolved("Self".to_string())),
            "i32" => Type::I32,
            "bool" | "i8" | "i16" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
                Type::Primitive(reference.name.clone())
            }
            "str" => Type::Str,
            "error" => Type::Error,
            "void" => Type::Void,
            "never" => Type::Never,
            name => resolved
                .type_symbol_by_name(name)
                .map(|symbol| {
                    let Some(alias_target) = &symbol.alias_target else {
                        return Type::Named(symbol.canonical_name.clone());
                    };
                    let canonical_name = symbol.canonical_name.clone();
                    if !resolving_aliases.insert(canonical_name.clone()) {
                        return Type::Named(canonical_name);
                    }
                    let resolved_alias = type_expr_to_type_inner(
                        alias_target,
                        resolved,
                        self_type,
                        resolving_aliases,
                    );
                    resolving_aliases.remove(&canonical_name);
                    resolved_alias
                })
                .unwrap_or_else(|| Type::Unresolved(name.to_string())),
        },
        TypeExpr::Generic(_) | TypeExpr::Pointer(_) | TypeExpr::Borrow(_) => {
            type_expr_display_with_self_type(ty, resolved, self_type)
                .map(Type::Named)
                .unwrap_or_else(|| Type::Unresolved(type_expr_display_lossy(ty)))
        }
        TypeExpr::View(ty) => Type::View {
            is_readwrite: ty.is_readwrite,
            element: Box::new(type_expr_to_type_inner(
                &ty.element,
                resolved,
                self_type,
                resolving_aliases,
            )),
        },
        TypeExpr::Array(ty) => Type::Array {
            element: Box::new(type_expr_to_type_inner(
                &ty.element,
                resolved,
                self_type,
                resolving_aliases,
            )),
            length: ty.length.value.clone(),
        },
        TypeExpr::Optional(ty) => Type::Optional(Box::new(type_expr_to_type_inner(
            &ty.inner,
            resolved,
            self_type,
            resolving_aliases,
        ))),
        TypeExpr::Fallible(ty) => Type::Fallible {
            success: Box::new(type_expr_to_type_inner(
                &ty.success,
                resolved,
                self_type,
                resolving_aliases,
            )),
            error: Box::new(type_expr_to_type_inner(
                &ty.error,
                resolved,
                self_type,
                resolving_aliases,
            )),
        },
    }
}

fn type_expr_display_with_self_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<String> {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "Self" => self_type.map(Type::display),
            "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
            | "isize" | "str" | "error" | "void" | "never" => Some(reference.name.clone()),
            name => resolved
                .type_symbol_by_name(name)
                .map(|symbol| symbol.canonical_name.clone()),
        },
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(|argument| type_expr_display_with_self_type(argument, resolved, self_type))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            let name = resolved
                .type_symbol_by_name(&generic.name)
                .map(|symbol| symbol.canonical_name.clone())?;
            Some(format!("{name}<{arguments}>"))
        }
        TypeExpr::Pointer(pointer) => Some(format!(
            "*{}",
            type_expr_display_with_self_type(&pointer.inner, resolved, self_type)?
        )),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => Some(format!(
            "&+{}",
            type_expr_display_with_self_type(&borrow.inner, resolved, self_type)?
        )),
        TypeExpr::Borrow(borrow) => Some(format!(
            "&{}",
            type_expr_display_with_self_type(&borrow.inner, resolved, self_type)?
        )),
        TypeExpr::View(view) if view.is_readwrite => Some(format!(
            "[+{}]",
            type_expr_display_with_self_type(&view.element, resolved, self_type)?
        )),
        TypeExpr::View(view) => Some(format!(
            "[{}]",
            type_expr_display_with_self_type(&view.element, resolved, self_type)?
        )),
        TypeExpr::Array(array) => Some(format!(
            "[{}; {}]",
            type_expr_display_with_self_type(&array.element, resolved, self_type)?,
            array.length.value
        )),
        TypeExpr::Optional(optional) => Some(format!(
            "{}?",
            type_expr_display_with_self_type(&optional.inner, resolved, self_type)?
        )),
        TypeExpr::Fallible(fallible) => Some(format!(
            "{}!",
            type_expr_display_with_self_type(&fallible.success, resolved, self_type)?
        )),
    }
}

pub(super) fn type_expr_display_lossy(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Reference(reference) => reference.name.clone(),
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(type_expr_display_lossy)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", generic.name)
        }
        TypeExpr::Pointer(pointer) => format!("*{}", type_expr_display_lossy(&pointer.inner)),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            format!("&+{}", type_expr_display_lossy(&borrow.inner))
        }
        TypeExpr::Borrow(borrow) => format!("&{}", type_expr_display_lossy(&borrow.inner)),
        TypeExpr::View(view) if view.is_readwrite => {
            format!("[+{}]", type_expr_display_lossy(&view.element))
        }
        TypeExpr::View(view) => format!("[{}]", type_expr_display_lossy(&view.element)),
        TypeExpr::Array(array) => {
            format!(
                "[{}; {}]",
                type_expr_display_lossy(&array.element),
                array.length.value
            )
        }
        TypeExpr::Optional(optional) => format!("{}?", type_expr_display_lossy(&optional.inner)),
        TypeExpr::Fallible(fallible) => format!("{}!", type_expr_display_lossy(&fallible.success)),
    }
}
