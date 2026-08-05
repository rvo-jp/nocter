use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::typecheck::model::Type;
use crate::typecheck::returns::type_contains_borrow_like;
use crate::typecheck::type_expr::type_expr_to_type_with_substitutions;
use std::collections::{HashMap, HashSet};

/// Returns whether values of `ty` can retain storage provenance.
///
/// Borrow checking deliberately ignores raw pointers, but result provenance
/// also describes owned allocation-backed values such as `String` and `Vec`.
/// Keeping this predicate separate prevents pointer-bearing ownership from
/// weakening the rules for source-level borrows.
pub(in crate::typecheck) fn type_may_carry_result_provenance(
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    type_contains_borrow_like(ty, resolved)
        || type_contains_pointer(ty, resolved, &mut HashSet::new())
}

fn type_contains_pointer(
    ty: &Type,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Callable(_) => false,
        Type::Closure(closure) => closure.captures.iter().any(|capture| {
            capture.mode != crate::ast::ClosureCaptureMode::Move
                || type_expr_contains_pointer(
                    &capture.ty,
                    resolved,
                    &HashMap::new(),
                    resolving_names,
                )
        }),
        Type::Pointer(_) => true,
        Type::Borrow { inner, .. } => type_contains_pointer(inner, resolved, resolving_names),
        Type::Named(name) => {
            type_symbol_contains_pointer(name, resolved, &HashMap::new(), resolving_names)
        }
        Type::Generic { name, arguments } => {
            let Some(symbol) = resolved.type_symbol_by_canonical_name(name) else {
                return false;
            };
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            type_symbol_contains_pointer(name, resolved, &substitutions, resolving_names)
        }
        Type::Array { element, .. } | Type::Optional(element) | Type::ArrayData { element } => {
            type_contains_pointer(element, resolved, resolving_names)
        }
        Type::Fallible { success, error } => {
            type_contains_pointer(success, resolved, resolving_names)
                || type_contains_pointer(error, resolved, resolving_names)
        }
        Type::I32
        | Type::Primitive(_)
        | Type::Str
        | Type::StrData
        | Type::View { .. }
        | Type::Void
        | Type::Never
        | Type::None
        | Type::Error
        | Type::Parameter(_)
        | Type::Unresolved(_)
        | Type::Unknown => false,
    }
}

fn type_symbol_contains_pointer(
    canonical_name: &str,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    if !resolving_names.insert(canonical_name.to_string()) {
        return false;
    }

    let result = resolved
        .type_symbol_by_canonical_name(canonical_name)
        .is_some_and(|symbol| match symbol.kind {
            TypeSymbolKind::Alias => symbol.alias_target.as_ref().is_some_and(|target| {
                type_expr_contains_pointer(target, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Struct => symbol.fields.iter().any(|field| {
                type_expr_contains_pointer(&field.ty, resolved, substitutions, resolving_names)
            }),
            TypeSymbolKind::Enum => symbol.variants.iter().any(|variant| {
                variant.payload.iter().any(|payload| {
                    type_expr_contains_pointer(
                        &payload.ty,
                        resolved,
                        substitutions,
                        resolving_names,
                    )
                })
            }),
            TypeSymbolKind::Interface => false,
        });

    resolving_names.remove(canonical_name);
    result
}

fn type_expr_contains_pointer(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    resolving_names: &mut HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Callable(_) => false,
        TypeExpr::Closure(closure) => closure.captures.iter().any(|capture| {
            capture.mode != crate::ast::ClosureCaptureMode::Move
                || type_expr_contains_pointer(&capture.ty, resolved, substitutions, resolving_names)
        }),
        TypeExpr::Pointer(_) => true,
        TypeExpr::Borrow(borrow) => {
            type_expr_contains_pointer(&borrow.inner, resolved, substitutions, resolving_names)
        }
        TypeExpr::View(view) => {
            type_expr_contains_pointer(&view.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_pointer(&array.element, resolved, substitutions, resolving_names)
        }
        TypeExpr::Optional(optional) => {
            type_expr_contains_pointer(&optional.inner, resolved, substitutions, resolving_names)
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_pointer(&fallible.success, resolved, substitutions, resolving_names)
                || type_expr_contains_pointer(
                    &fallible.error,
                    resolved,
                    substitutions,
                    resolving_names,
                )
        }
        TypeExpr::Reference(reference) => {
            substitutions
                .get(&reference.name)
                .is_some_and(|ty| type_contains_pointer(ty, resolved, resolving_names))
                || resolved
                    .type_symbol_by_reference_name(&reference.name)
                    .is_some_and(|symbol| {
                        type_symbol_contains_pointer(
                            &symbol.canonical_name,
                            resolved,
                            &HashMap::new(),
                            resolving_names,
                        )
                    })
        }
        TypeExpr::Generic(generic) => {
            if let Some(ty) = substitutions.get(&generic.name) {
                return type_contains_pointer(ty, resolved, resolving_names);
            }
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return false;
            };
            let nested_substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().map(|argument| {
                    type_expr_to_type_with_substitutions(argument, resolved, None, substitutions)
                }))
                .collect();
            type_symbol_contains_pointer(
                &symbol.canonical_name,
                resolved,
                &nested_substitutions,
                resolving_names,
            )
        }
    }
}
