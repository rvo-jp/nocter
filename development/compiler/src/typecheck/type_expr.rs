use super::model::{CallableParameterType, CallableType, OpaqueType, Type, TypeEnvironment};
use crate::ast::TypeExpr;
use crate::resolve::ResolveOutput;
use std::collections::{HashMap, HashSet};

pub(super) use crate::ast::canonical_type_expr;

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
        TypeExpr::Callable(expected_callable) => {
            let Type::Callable(actual_callable) = actual else {
                return;
            };
            for (expected, actual) in expected_callable
                .parameters
                .iter()
                .zip(&actual_callable.parameters)
            {
                infer_type_expr_substitutions(
                    &expected.ty,
                    &actual.ty,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
            infer_type_expr_substitutions(
                &expected_callable.return_type,
                &actual_callable.return_type,
                resolved,
                self_type,
                parameters,
                substitutions,
            );
        }
        TypeExpr::Closure(expected_closure) => {
            let Type::Closure(actual_closure) = actual else {
                return;
            };
            for (expected, actual) in expected_closure
                .parameters
                .iter()
                .zip(actual_closure.parameters.iter())
            {
                let actual = type_expr_to_type_with_substitutions(
                    actual,
                    resolved,
                    self_type,
                    substitutions,
                );
                infer_type_expr_substitutions(
                    expected,
                    &actual,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
            let actual_return = type_expr_to_type_with_substitutions(
                &actual_closure.return_type,
                resolved,
                self_type,
                substitutions,
            );
            infer_type_expr_substitutions(
                &expected_closure.return_type,
                &actual_return,
                resolved,
                self_type,
                parameters,
                substitutions,
            );
        }
        TypeExpr::Opaque(_) => {}
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            if let Some(expected_self) = self_type {
                infer_type_substitutions(expected_self, actual, parameters, substitutions);
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
        TypeExpr::Projection(_) => {}
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
        TypeExpr::Callable(callable) => Type::Callable(CallableType {
            span: callable.span,
            capability: callable.capability,
            parameters: callable
                .parameters
                .iter()
                .map(|parameter| CallableParameterType {
                    name: parameter.name.clone(),
                    name_span: parameter.name_span,
                    ty: type_expr_to_type_inner(
                        &parameter.ty,
                        resolved,
                        self_type,
                        substitutions,
                        resolving_aliases,
                    ),
                })
                .collect(),
            return_type: Box::new(type_expr_to_type_inner(
                &callable.return_type,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
            result_provenance: callable.result_provenance.clone(),
        }),
        TypeExpr::Closure(closure) => Type::Closure(closure.clone()),
        TypeExpr::Opaque(opaque) => Type::Opaque(OpaqueType {
            identity: opaque.some_span,
            interface: Box::new(type_expr_to_type_inner(
                &opaque.interface,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            )),
            associated_bindings: opaque
                .associated_bindings
                .iter()
                .map(|binding| {
                    (
                        binding.name.clone(),
                        type_expr_to_type_inner(
                            &binding.value,
                            resolved,
                            self_type,
                            substitutions,
                            resolving_aliases,
                        ),
                    )
                })
                .collect(),
            witness: opaque.witness.as_ref().map(|witness| {
                Box::new(type_expr_to_type_inner(
                    witness,
                    resolved,
                    self_type,
                    substitutions,
                    resolving_aliases,
                ))
            }),
        }),
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
                    Type::Unresolved(canonical_type_expr(ty))
                }
                (_, inner_type) => Type::Borrow {
                    is_readwrite: borrow.is_readwrite,
                    inner: Box::new(inner_type),
                },
            }
        }
        TypeExpr::Generic(generic) => {
            if substitutions.contains_key(&generic.name) {
                return Type::Unresolved(generic.name.clone());
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return Type::Unresolved(canonical_type_expr(ty));
            };
            if symbol.generic_arity != generic.arguments.len() {
                return Type::Unresolved(canonical_type_expr(ty));
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
        TypeExpr::Projection(projection) => {
            let base = type_expr_to_type_inner(
                &projection.base,
                resolved,
                self_type,
                substitutions,
                resolving_aliases,
            );
            super::associated_types::normalize_projection(base, &projection.name, resolved)
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

fn infer_type_substitutions(
    expected: &Type,
    actual: &Type,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    match (expected, actual) {
        (Type::Parameter(parameter), actual) if parameters.contains(parameter.as_str()) => {
            merge_inferred_substitution(parameter, actual, substitutions);
        }
        (
            Type::Generic {
                name: expected_name,
                arguments: expected_arguments,
            },
            Type::Generic {
                name: actual_name,
                arguments: actual_arguments,
            },
        ) if expected_name == actual_name && expected_arguments.len() == actual_arguments.len() => {
            for (expected, actual) in expected_arguments.iter().zip(actual_arguments) {
                infer_type_substitutions(expected, actual, parameters, substitutions);
            }
        }
        (
            Type::Projection {
                base: expected_base,
                member: expected_member,
            },
            Type::Projection {
                base: actual_base,
                member: actual_member,
            },
        ) if expected_member == actual_member => {
            infer_type_substitutions(expected_base, actual_base, parameters, substitutions);
        }
        (Type::Pointer(expected), Type::Pointer(actual))
        | (Type::Optional(expected), Type::Optional(actual)) => {
            infer_type_substitutions(expected, actual, parameters, substitutions);
        }
        (Type::ArrayData { element: expected }, Type::ArrayData { element: actual })
        | (
            Type::View {
                is_readwrite: false,
                element: expected,
            },
            Type::View {
                is_readwrite: false,
                element: actual,
            },
        )
        | (
            Type::View {
                is_readwrite: true,
                element: expected,
            },
            Type::View {
                is_readwrite: true,
                element: actual,
            },
        ) => infer_type_substitutions(expected, actual, parameters, substitutions),
        (
            Type::Array {
                element: expected, ..
            },
            Type::Array {
                element: actual, ..
            },
        ) => infer_type_substitutions(expected, actual, parameters, substitutions),
        (
            Type::Fallible {
                success: expected_success,
                error: expected_error,
            },
            Type::Fallible {
                success: actual_success,
                error: actual_error,
            },
        ) => {
            infer_type_substitutions(expected_success, actual_success, parameters, substitutions);
            infer_type_substitutions(expected_error, actual_error, parameters, substitutions);
        }
        _ => {}
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
        Type::Borrow {
            is_readwrite: actual_readwrite,
            inner,
        } if *actual_readwrite == is_readwrite => Some(normalize_type_parameters(
            inner.as_ref().clone(),
            parameters,
        )),
        _ => None,
    }
}

fn normalize_type_parameters(ty: Type, parameters: &HashSet<&str>) -> Type {
    match ty {
        Type::Named(name) if parameters.contains(name.as_str()) => Type::Parameter(name),
        Type::Generic { name, arguments } => Type::Generic {
            name,
            arguments: arguments
                .into_iter()
                .map(|argument| normalize_type_parameters(argument, parameters))
                .collect(),
        },
        Type::Pointer(inner) => {
            Type::Pointer(Box::new(normalize_type_parameters(*inner, parameters)))
        }
        Type::Borrow {
            is_readwrite,
            inner,
        } => Type::Borrow {
            is_readwrite,
            inner: Box::new(normalize_type_parameters(*inner, parameters)),
        },
        Type::Optional(inner) => {
            Type::Optional(Box::new(normalize_type_parameters(*inner, parameters)))
        }
        Type::Fallible { success, error } => Type::Fallible {
            success: Box::new(normalize_type_parameters(*success, parameters)),
            error: Box::new(normalize_type_parameters(*error, parameters)),
        },
        Type::View {
            is_readwrite,
            element,
        } => Type::View {
            is_readwrite,
            element: Box::new(normalize_type_parameters(*element, parameters)),
        },
        Type::ArrayData { element } => Type::ArrayData {
            element: Box::new(normalize_type_parameters(*element, parameters)),
        },
        Type::Array { element, length } => Type::Array {
            element: Box::new(normalize_type_parameters(*element, parameters)),
            length,
        },
        _ => ty,
    }
}
