use super::model::{Type, TypeEnvironment};
use crate::ast::TypeExpr;
use crate::resolve::ResolveOutput;
use std::collections::{HashMap, HashSet};

pub(super) fn type_expr_to_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Type {
    type_expr_to_type_with_self_type(ty, resolved, None)
}

pub(super) fn type_expr_to_type_in_environment(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    type_expr_to_type_with_substitutions(
        ty,
        resolved,
        environment.self_type(),
        &environment.generic_parameter_substitutions(),
    )
}

pub(super) fn type_expr_to_type_with_self_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Type {
    type_expr_to_type_with_substitutions(ty, resolved, self_type, &HashMap::new())
}

pub(super) fn type_expr_to_type_with_substitutions(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    substitutions: &HashMap<String, Type>,
) -> Type {
    type_expr_to_type_inner(ty, resolved, self_type, substitutions, &mut HashSet::new())
}

fn type_expr_to_type_inner(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    substitutions: &HashMap<String, Type>,
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
            "str" => Type::StrData,
            "error" => Type::Error,
            "void" => Type::Void,
            "never" => Type::Never,
            name if substitutions.contains_key(name) => substitutions
                .get(name)
                .cloned()
                .unwrap_or_else(|| Type::Unresolved(name.to_string())),
            name => resolved
                .type_symbol_by_reference_name(name)
                .map(|symbol| {
                    if symbol.generic_arity > 0 {
                        return Type::Unresolved(name.to_string());
                    }
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
                        substitutions,
                        resolving_aliases,
                    );
                    resolving_aliases.remove(&canonical_name);
                    resolved_alias
                })
                .unwrap_or_else(|| Type::Unresolved(name.to_string())),
        },
        TypeExpr::Borrow(borrow) => {
            let inner_type = type_expr_to_type_inner(
                &borrow.inner,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            );
            match (borrow.is_readwrite, inner_type) {
                (false, Type::StrData) => Type::Str,
                (_, Type::ArrayData { element }) => Type::View {
                    is_readwrite: borrow.is_readwrite,
                    element,
                },
                (_, inner_type) if inner_type.is_unknown_or_unresolved() => {
                    Type::Unresolved(type_expr_display_lossy(ty))
                }
                (_, inner_type) => Type::Named(format!(
                    "{}{}",
                    if borrow.is_readwrite { "&+" } else { "&" },
                    inner_type.display()
                )),
            }
        }
        TypeExpr::Generic(generic) => {
            if substitutions.contains_key(&generic.name) {
                return Type::Unresolved(generic.name.clone());
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return Type::Unresolved(type_expr_display_lossy(ty));
            };
            if symbol.generic_arity != generic.arguments.len() {
                return Type::Unresolved(type_expr_display_lossy(ty));
            }
            let arguments = generic
                .arguments
                .iter()
                .map(|argument| {
                    type_expr_to_type_inner(
                        argument,
                        resolved,
                        self_type,
                        substitutions,
                        resolving_aliases,
                    )
                })
                .collect::<Vec<_>>();

            let Some(alias_target) = &symbol.alias_target else {
                return Type::Generic {
                    name: symbol.canonical_name.clone(),
                    arguments,
                };
            };

            let canonical_name = symbol.canonical_name.clone();
            if !resolving_aliases.insert(canonical_name.clone()) {
                return Type::Generic {
                    name: canonical_name,
                    arguments,
                };
            }
            let mut alias_substitutions = substitutions.clone();
            for (parameter, argument) in symbol.generic_parameters.iter().zip(arguments.iter()) {
                alias_substitutions.insert(parameter.clone(), argument.clone());
            }
            let resolved_alias = type_expr_to_type_inner(
                alias_target,
                resolved,
                self_type,
                &alias_substitutions,
                resolving_aliases,
            );
            resolving_aliases.remove(&canonical_name);
            resolved_alias
        }
        TypeExpr::Pointer(pointer) => Type::Pointer(Box::new(type_expr_to_type_inner(
            &pointer.inner,
            resolved,
            self_type,
            substitutions,
            resolving_aliases,
        ))),
        TypeExpr::View(ty) => Type::ArrayData {
            element: Box::new(type_expr_to_type_inner(
                &ty.element,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
        },
        TypeExpr::Array(ty) => Type::Array {
            element: Box::new(type_expr_to_type_inner(
                &ty.element,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
            length: ty.length.value.clone(),
        },
        TypeExpr::Optional(ty) => Type::Optional(Box::new(type_expr_to_type_inner(
            &ty.inner,
            resolved,
            self_type,
            substitutions,
            resolving_aliases,
        ))),
        TypeExpr::Fallible(ty) => Type::Fallible {
            success: Box::new(type_expr_to_type_inner(
                &ty.success,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
            error: Box::new(type_expr_to_type_inner(
                &ty.error,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
        },
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
            format!("&+[{}]", type_expr_display_lossy(&view.element))
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
