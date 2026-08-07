//! Type-directed projection of lossless provenance for public contracts.
//!
//! The flow model intentionally records scalar dataflow inside aggregates so
//! ownership and mutation analyses do not lose information. A source `from`
//! contract describes only storage-bearing result projections, however. This
//! module is the single boundary that prevents scalar fields from becoming
//! accidental lifetime requirements.

use super::{StorageOrigin, ValueProvenance, type_may_carry_result_provenance};
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::typecheck::allocation::allocator_capability_kind;
use crate::typecheck::model::Type;
use crate::typecheck::type_expr::type_expr_to_type_with_substitutions;
use std::collections::HashMap;

pub(super) fn external_origins_satisfy(
    actual: &ValueProvenance,
    allowed: &[StorageOrigin],
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    match actual {
        ValueProvenance::Independent => true,
        ValueProvenance::Origins(origins) => origins.iter().all(|origin| match origin {
            StorageOrigin::Allocated(domain) => match domain.allocation_domain() {
                StorageOrigin::Static | StorageOrigin::CurrentAllocationContext => true,
                StorageOrigin::Input(input) | StorageOrigin::InputWithCurrentFallback(input) => {
                    allowed.contains(&StorageOrigin::Input(*input))
                }
                StorageOrigin::Scope { .. }
                | StorageOrigin::Region { .. }
                | StorageOrigin::Unknown => false,
                StorageOrigin::Allocated(_) => unreachable!("allocation domains are unwrapped"),
            },
            StorageOrigin::Static => true,
            StorageOrigin::CurrentAllocationContext | StorageOrigin::Input(_) => {
                allowed.contains(origin)
            }
            StorageOrigin::InputWithCurrentFallback(input) => {
                allowed.contains(&StorageOrigin::Input(*input))
            }
            StorageOrigin::Scope { .. } | StorageOrigin::Region { .. } | StorageOrigin::Unknown => {
                false
            }
        }),
        ValueProvenance::Fallible { success, error } => match ty {
            Type::Fallible {
                success: success_ty,
                error: error_ty,
            } => {
                success.as_deref().is_none_or(|value| {
                    external_origins_satisfy(value, allowed, success_ty, resolved)
                }) && error.as_deref().is_none_or(|value| {
                    external_origins_satisfy(value, allowed, error_ty, resolved)
                })
            }
            _ => conservative_children_satisfy(actual, allowed, ty, resolved),
        },
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            fallback
                .as_deref()
                .is_none_or(|value| external_origins_satisfy(value, allowed, ty, resolved))
                && fields.iter().all(|(name, value)| {
                    aggregate_field_type(ty, name, resolved).map_or_else(
                        || external_origins_satisfy(value, allowed, ty, resolved),
                        |field_ty| {
                            !type_carries_storage(&field_ty, resolved)
                                || external_origins_satisfy(value, allowed, &field_ty, resolved)
                        },
                    )
                })
                && elements.values().all(|value| {
                    aggregate_element_type(ty).map_or_else(
                        || external_origins_satisfy(value, allowed, ty, resolved),
                        |element_ty| {
                            !type_carries_storage(element_ty, resolved)
                                || external_origins_satisfy(value, allowed, element_ty, resolved)
                        },
                    )
                })
        }
    }
}

#[cfg(test)]
pub(in crate::typecheck) fn result_contains_allocation(
    actual: &ValueProvenance,
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    match actual {
        ValueProvenance::Independent => false,
        ValueProvenance::Origins(origins) => {
            type_carries_storage(ty, resolved) && origins.iter().any(StorageOrigin::is_allocated)
        }
        ValueProvenance::Fallible { success, error } => match ty {
            Type::Fallible {
                success: success_ty,
                error: error_ty,
            } => {
                success
                    .as_deref()
                    .is_some_and(|value| result_contains_allocation(value, success_ty, resolved))
                    || error
                        .as_deref()
                        .is_some_and(|value| result_contains_allocation(value, error_ty, resolved))
            }
            _ => conservative_children_contain_allocation(actual, ty, resolved),
        },
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            fallback
                .as_deref()
                .is_some_and(|value| result_contains_allocation(value, ty, resolved))
                || fields.iter().any(|(name, value)| {
                    aggregate_field_type(ty, name, resolved).map_or_else(
                        || result_contains_allocation(value, ty, resolved),
                        |field_ty| result_contains_allocation(value, &field_ty, resolved),
                    )
                })
                || elements.values().any(|value| {
                    aggregate_element_type(ty).map_or_else(
                        || result_contains_allocation(value, ty, resolved),
                        |element_ty| result_contains_allocation(value, element_ty, resolved),
                    )
                })
        }
    }
}

#[cfg(test)]
fn conservative_children_contain_allocation(
    actual: &ValueProvenance,
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    match actual {
        ValueProvenance::Fallible { success, error } => {
            success
                .as_deref()
                .is_some_and(|value| result_contains_allocation(value, ty, resolved))
                || error
                    .as_deref()
                    .is_some_and(|value| result_contains_allocation(value, ty, resolved))
        }
        _ => unreachable!("only outcome provenance reaches the conservative fallback"),
    }
}

fn conservative_children_satisfy(
    actual: &ValueProvenance,
    allowed: &[StorageOrigin],
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    match actual {
        ValueProvenance::Fallible { success, error } => {
            success
                .as_deref()
                .is_none_or(|value| external_origins_satisfy(value, allowed, ty, resolved))
                && error
                    .as_deref()
                    .is_none_or(|value| external_origins_satisfy(value, allowed, ty, resolved))
        }
        _ => unreachable!("only outcome provenance reaches the conservative fallback"),
    }
}

fn type_carries_storage(ty: &Type, resolved: &ResolveOutput) -> bool {
    type_may_carry_result_provenance(ty, resolved)
        || allocator_capability_kind(ty, resolved).is_some()
        || matches!(ty, Type::Parameter(_) | Type::Unresolved(_) | Type::Unknown)
}

fn aggregate_element_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Array { element, .. }
        | Type::ArrayData { element }
        | Type::Optional(element)
        | Type::View { element, .. } => Some(element),
        _ => None,
    }
}

fn aggregate_field_type(ty: &Type, name: &str, resolved: &ResolveOutput) -> Option<Type> {
    let (canonical_name, substitutions) = match ty {
        Type::Named(name) => (name.as_str(), HashMap::new()),
        Type::Generic { name, arguments } => {
            let symbol = resolved.type_symbol_by_canonical_name(name)?;
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            (name.as_str(), substitutions)
        }
        _ => return None,
    };
    let symbol = resolved.type_symbol_by_canonical_name(canonical_name)?;
    match symbol.kind {
        TypeSymbolKind::Struct => {
            let field = symbol.fields.iter().find(|field| field.name == name)?;
            Some(type_expr_to_type_with_substitutions(
                &field.ty,
                resolved,
                None,
                &substitutions,
            ))
        }
        TypeSymbolKind::Alias => symbol.alias_target.as_ref().and_then(|target| {
            let target =
                type_expr_to_type_with_substitutions(target, resolved, None, &substitutions);
            aggregate_field_type(&target, name, resolved)
        }),
        TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::source::{ByteSpan, SourceId, SourceMap};
    use crate::typecheck::provenance::InputId;
    use std::collections::BTreeMap;

    #[test]
    fn ignores_scalar_aggregate_dataflow_but_checks_storage_fields() {
        let text = "struct Pair { value: &i32, tag: usize }\n";
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text);
        let tokens = lex(&sources, source);
        let ast = parse(&sources, source, &tokens.tokens).ast.unwrap();
        let resolved = resolve(&sources, &ast);
        let allowed = InputId::declared_at(ByteSpan::new(SourceId::new(0), 1, 2));
        let rejected = InputId::declared_at(ByteSpan::new(SourceId::new(0), 3, 4));
        let actual = ValueProvenance::Aggregate {
            fallback: None,
            fields: BTreeMap::from([
                ("value".to_string(), ValueProvenance::input(allowed)),
                ("tag".to_string(), ValueProvenance::input(rejected)),
            ]),
            elements: BTreeMap::new(),
        };

        assert!(external_origins_satisfy(
            &actual,
            &[StorageOrigin::Input(allowed)],
            &Type::Named("Pair".to_string()),
            &resolved,
        ));
    }

    #[test]
    fn ignores_allocations_in_scalar_outcome_projections() {
        let text = "struct Error { message: &str }\n";
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text);
        let tokens = lex(&sources, source);
        let ast = parse(&sources, source, &tokens.tokens).ast.unwrap();
        let resolved = resolve(&sources, &ast);
        let actual = ValueProvenance::Fallible {
            success: Some(Box::new(
                ValueProvenance::current_allocation_context().allocated(),
            )),
            error: Some(Box::new(ValueProvenance::static_storage())),
        };
        let ty = Type::Fallible {
            success: Box::new(Type::Primitive("usize".into())),
            error: Box::new(Type::Error),
        };

        assert!(!result_contains_allocation(&actual, &ty, &resolved));
    }
}
