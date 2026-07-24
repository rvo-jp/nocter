use super::model::{Type, TypeEnvironment};
use crate::ast::TypeExpr;
use crate::resolve::ResolveOutput;
use std::collections::{HashMap, HashSet};

pub(super) use crate::ast::type_expr_display_lossy;

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

pub(super) fn infer_type_expr_substitutions(
    expected: &TypeExpr,
    actual: &Type,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    match expected {
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            if self_type.is_some_and(|self_type| self_type == actual) {
                return;
            }
        }
        TypeExpr::Reference(reference) if parameters.contains(reference.name.as_str()) => {
            merge_inferred_substitution(&reference.name, actual, substitutions);
        }
        TypeExpr::Pointer(pointer) => {
            if let Type::Pointer(actual_inner) = actual {
                infer_type_expr_substitutions(
                    &pointer.inner,
                    actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Borrow(borrow) => {
            if let Some(actual_inner) =
                borrowed_actual_inner_type(actual, borrow.is_readwrite, parameters)
            {
                infer_type_expr_substitutions(
                    &borrow.inner,
                    &actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::View(view) => {
            if let Type::ArrayData { element } = actual {
                infer_type_expr_substitutions(
                    &view.element,
                    element,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Array(array) => {
            if let Type::Array { element, .. } = actual {
                infer_type_expr_substitutions(
                    &array.element,
                    element,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Optional(optional) => {
            if let Type::Optional(actual_inner) = actual {
                infer_type_expr_substitutions(
                    &optional.inner,
                    actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Fallible(fallible) => {
            if let Type::Fallible { success, error } = actual {
                infer_type_expr_substitutions(
                    &fallible.success,
                    success,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
                infer_type_expr_substitutions(
                    &fallible.error,
                    error,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Generic(generic) => {
            if let Some(expected_arguments) =
                expected_generic_parts(generic, actual, resolved, self_type)
                && expected_arguments.len() == generic.arguments.len()
            {
                for (expected_argument, actual_argument) in
                    generic.arguments.iter().zip(expected_arguments.iter())
                {
                    infer_type_expr_substitutions(
                        expected_argument,
                        actual_argument,
                        resolved,
                        self_type,
                        parameters,
                        substitutions,
                    );
                }
            }
        }
        TypeExpr::Reference(_) => {}
    }
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

fn expected_generic_parts(
    generic: &crate::ast::GenericType,
    actual: &Type,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Vec<Type>> {
    let expected_name = if generic.name == "Self" {
        self_type?.nominal_name()?.to_string()
    } else {
        resolved
            .type_symbol_by_reference_name(&generic.name)
            .map(|symbol| symbol.canonical_name.clone())
            .unwrap_or_else(|| generic.name.clone())
    };

    match actual {
        Type::Generic { name, arguments } if *name == expected_name => Some(arguments.clone()),
        _ => None,
    }
}

fn merge_inferred_substitution(
    name: &str,
    actual: &Type,
    substitutions: &mut HashMap<String, Type>,
) {
    if let Some(existing) = substitutions.get(name) {
        if let Some(merged) = merge_substitution_types(existing, actual) {
            substitutions.insert(name.to_string(), merged);
        }
    } else {
        substitutions.insert(name.to_string(), actual.clone());
    }
}

fn merge_substitution_types(existing: &Type, actual: &Type) -> Option<Type> {
    if existing == actual {
        return Some(existing.clone());
    }

    match (existing, actual) {
        (Type::Parameter(_), _) => Some(actual.clone()),
        (_, Type::Parameter(_)) => Some(existing.clone()),
        (Type::Pointer(existing), Type::Pointer(actual)) => {
            merge_substitution_types(existing, actual).map(|inner| Type::Pointer(Box::new(inner)))
        }
        (Type::Optional(existing), Type::Optional(actual)) => {
            merge_substitution_types(existing, actual).map(|inner| Type::Optional(Box::new(inner)))
        }
        (
            Type::View {
                is_readwrite: existing_readwrite,
                element: existing,
            },
            Type::View {
                is_readwrite: actual_readwrite,
                element: actual,
            },
        ) if existing_readwrite == actual_readwrite => merge_substitution_types(existing, actual)
            .map(|element| Type::View {
                is_readwrite: *existing_readwrite,
                element: Box::new(element),
            }),
        (Type::ArrayData { element: existing }, Type::ArrayData { element: actual }) => {
            merge_substitution_types(existing, actual).map(|element| Type::ArrayData {
                element: Box::new(element),
            })
        }
        (
            Type::Array {
                element: existing,
                length: existing_length,
            },
            Type::Array {
                element: actual,
                length: actual_length,
            },
        ) if existing_length == actual_length => {
            merge_substitution_types(existing, actual).map(|element| Type::Array {
                element: Box::new(element),
                length: existing_length.clone(),
            })
        }
        (
            Type::Fallible {
                success: existing_success,
                error: existing_error,
            },
            Type::Fallible {
                success: actual_success,
                error: actual_error,
            },
        ) => {
            let success = merge_substitution_types(existing_success, actual_success)?;
            let error = merge_substitution_types(existing_error, actual_error)?;
            Some(Type::Fallible {
                success: Box::new(success),
                error: Box::new(error),
            })
        }
        (
            Type::Generic {
                name: existing_name,
                arguments: existing_arguments,
            },
            Type::Generic {
                name: actual_name,
                arguments: actual_arguments,
            },
        ) if existing_name == actual_name && existing_arguments.len() == actual_arguments.len() => {
            let arguments = existing_arguments
                .iter()
                .zip(actual_arguments.iter())
                .map(|(existing, actual)| merge_substitution_types(existing, actual))
                .collect::<Option<Vec<_>>>()?;
            Some(Type::Generic {
                name: existing_name.clone(),
                arguments,
            })
        }
        _ => None,
    }
}

fn borrowed_actual_inner_type(
    actual: &Type,
    is_readwrite: bool,
    parameters: &HashSet<&str>,
) -> Option<Type> {
    match actual {
        Type::Str if !is_readwrite => Some(Type::StrData),
        Type::View {
            is_readwrite: actual_readwrite,
            element,
        } if *actual_readwrite == is_readwrite => Some(Type::ArrayData {
            element: element.clone(),
        }),
        Type::Named(name) if is_readwrite => name
            .strip_prefix("&+")
            .map(simple_type_from_display_name)
            .map(|ty| normalize_display_type_parameters(ty, parameters)),
        Type::Named(name) if !is_readwrite => name
            .strip_prefix('&')
            .map(simple_type_from_display_name)
            .map(|ty| normalize_display_type_parameters(ty, parameters)),
        _ => None,
    }
}

fn normalize_display_type_parameters(ty: Type, parameters: &HashSet<&str>) -> Type {
    match ty {
        Type::Named(name) if parameters.contains(name.as_str()) => Type::Parameter(name),
        Type::Generic { name, arguments } => Type::Generic {
            name,
            arguments: arguments
                .into_iter()
                .map(|argument| normalize_display_type_parameters(argument, parameters))
                .collect(),
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(normalize_display_type_parameters(
            *inner, parameters,
        ))),
        Type::Optional(inner) => Type::Optional(Box::new(normalize_display_type_parameters(
            *inner, parameters,
        ))),
        Type::Fallible { success, error } => Type::Fallible {
            success: Box::new(normalize_display_type_parameters(*success, parameters)),
            error: Box::new(normalize_display_type_parameters(*error, parameters)),
        },
        Type::View {
            is_readwrite,
            element,
        } => Type::View {
            is_readwrite,
            element: Box::new(normalize_display_type_parameters(*element, parameters)),
        },
        Type::ArrayData { element } => Type::ArrayData {
            element: Box::new(normalize_display_type_parameters(*element, parameters)),
        },
        Type::Array { element, length } => Type::Array {
            element: Box::new(normalize_display_type_parameters(*element, parameters)),
            length,
        },
        _ => ty,
    }
}

pub(super) fn simple_type_from_display_name(name: &str) -> Type {
    let name = name.trim();
    match name {
        "i32" => Type::I32,
        "bool" | "i8" | "i16" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
            Type::Primitive(name.to_string())
        }
        "str" => Type::StrData,
        "&str" => Type::Str,
        "error" => Type::Error,
        "void" => Type::Void,
        "never" => Type::Never,
        name if name.ends_with('?') => Type::Optional(Box::new(simple_type_from_display_name(
            &name[..name.len() - 1],
        ))),
        name if name.ends_with('!') => Type::Fallible {
            success: Box::new(simple_type_from_display_name(&name[..name.len() - 1])),
            error: Box::new(Type::Error),
        },
        name if name.starts_with('*') => {
            Type::Pointer(Box::new(simple_type_from_display_name(&name[1..])))
        }
        name if name.starts_with("&+[") && name.ends_with(']') => Type::View {
            is_readwrite: true,
            element: Box::new(simple_type_from_display_name(&name[3..name.len() - 1])),
        },
        name if name.starts_with("&[") && name.ends_with(']') => Type::View {
            is_readwrite: false,
            element: Box::new(simple_type_from_display_name(&name[2..name.len() - 1])),
        },
        name if name.starts_with('[') && name.ends_with(']') => {
            let content = &name[1..name.len() - 1];
            if let Some((element, length)) = split_top_level_array_parts(content) {
                Type::Array {
                    element: Box::new(simple_type_from_display_name(element)),
                    length: length.to_string(),
                }
            } else {
                Type::ArrayData {
                    element: Box::new(simple_type_from_display_name(content)),
                }
            }
        }
        name => parse_generic_display_type(name).unwrap_or_else(|| Type::Named(name.to_string())),
    }
}

fn parse_generic_display_type(name: &str) -> Option<Type> {
    let open = name.find('<')?;
    let close = name.rfind('>')?;
    if close != name.len() - 1 || close <= open {
        return None;
    }
    let arguments = split_top_level_type_arguments(&name[open + 1..close])
        .into_iter()
        .map(simple_type_from_display_name)
        .collect();
    Some(Type::Generic {
        name: name[..open].trim().to_string(),
        arguments,
    })
}

fn split_top_level_array_parts(content: &str) -> Option<(&str, &str)> {
    let mut angle_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (index, ch) in content.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ';' if angle_depth == 0 && bracket_depth == 0 => {
                return Some((content[..index].trim(), content[index + 1..].trim()));
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_type_arguments(arguments: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in arguments.char_indices() {
        match ch {
            '<' | '[' | '(' => depth += 1,
            '>' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(arguments[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    result.push(arguments[start..].trim());
    result
}
