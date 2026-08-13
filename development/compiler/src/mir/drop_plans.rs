//! Target-independent destruction plans owned by checked MIR.
//!
//! Plans retain semantic destructor identities and aggregate structure. They
//! deliberately contain neither linker names nor machine-IR call targets;
//! those are backend projections of `DefId`.

use super::DropPlanId;
use crate::ast::{TypeExpr, substitute_type_expr_parameters};
use crate::resolve::{ResolveOutput, ResolvedSources, TypeSymbolKind};
use crate::semantic::{DefId, TyId};
use crate::typecheck::TypedHir;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DropPlan {
    Noop,
    Direct {
        destructor: DefId,
    },
    Struct {
        destructor: Option<DefId>,
        fields: Vec<DropPlanField>,
    },
    Array {
        length: u64,
        element_ty: TyId,
        element: DropPlanId,
    },
    Enum {
        variants: Vec<DropPlanVariant>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DropPlanField {
    pub(crate) index: usize,
    pub(crate) ty: TyId,
    pub(crate) plan: DropPlanId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DropPlanVariant {
    pub(crate) definition: DefId,
    pub(crate) fields: Vec<DropPlanField>,
}

pub(super) fn build(
    ty: &TypeExpr,
    fallback: &ResolveOutput,
    sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    plans: &mut Vec<DropPlan>,
) -> Option<DropPlanId> {
    build_inner(ty, fallback, sources, typed_hir, plans, &mut HashSet::new())
}

pub(super) fn is_supported(
    ty: &TypeExpr,
    fallback: &ResolveOutput,
    sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    build(ty, fallback, sources, typed_hir, &mut Vec::new()).is_some()
}

fn build_inner(
    ty: &TypeExpr,
    fallback: &ResolveOutput,
    sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    plans: &mut Vec<DropPlan>,
    resolving: &mut HashSet<String>,
) -> Option<DropPlanId> {
    if crate::typecheck::type_expr_is_copy(ty, resolver_for(ty, fallback, sources)) == Some(true) {
        return None;
    }
    match ty {
        TypeExpr::Opaque(opaque) => build_inner(
            opaque.witness.as_deref()?,
            fallback,
            sources,
            typed_hir,
            plans,
            resolving,
        ),
        TypeExpr::Projection(_) => {
            let resolved = resolver_for(ty, fallback, sources);
            let normalized = crate::typecheck::normalize_associated_type_expr(ty, resolved)?;
            build_inner(&normalized, fallback, sources, typed_hir, plans, resolving)
        }
        TypeExpr::Array(array) => {
            let element = build_inner(
                &array.element,
                fallback,
                sources,
                typed_hir,
                plans,
                resolving,
            )?;
            let element_ty = typed_hir.type_id(&array.element)?;
            let length = crate::literals::decode_integer_literal_value(&array.length.value)
                .and_then(|value| u64::try_from(value).ok())?;
            push(
                plans,
                DropPlan::Array {
                    length,
                    element_ty,
                    element,
                },
            )
        }
        TypeExpr::Reference(reference) => build_nominal(
            ty,
            &reference.name,
            HashMap::new(),
            fallback,
            sources,
            typed_hir,
            plans,
            resolving,
        ),
        TypeExpr::Generic(generic) => {
            let resolved = resolver_for(ty, fallback, sources);
            let symbol = type_symbol(resolved, &generic.name)?;
            if symbol.generic_parameters.len() != generic.arguments.len() {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            build_nominal(
                ty,
                &generic.name,
                substitutions,
                fallback,
                sources,
                typed_hir,
                plans,
                resolving,
            )
        }
        TypeExpr::Optional(_)
        | TypeExpr::Fallible(_)
        | TypeExpr::Callable(_)
        | TypeExpr::Closure(_)
        | TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_nominal(
    authored_ty: &TypeExpr,
    name: &str,
    substitutions: HashMap<String, TypeExpr>,
    fallback: &ResolveOutput,
    sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    plans: &mut Vec<DropPlan>,
    resolving: &mut HashSet<String>,
) -> Option<DropPlanId> {
    let resolved = resolver_for(authored_ty, fallback, sources);
    let (definition, symbol) = resolved
        .type_symbol_definition_by_reference_name(name)
        .or_else(|| {
            name.rsplit_once('.')
                .and_then(|(_, short)| resolved.type_symbol_definition_by_reference_name(short))
        })?;
    if !resolving.insert(symbol.canonical_name.clone()) {
        return None;
    }
    if symbol.kind == TypeSymbolKind::Alias {
        let target = substitute_type_expr_parameters(symbol.alias_target.as_ref()?, &substitutions);
        let result = build_inner(&target, fallback, sources, typed_hir, plans, resolving);
        resolving.remove(&symbol.canonical_name);
        return result;
    }
    if symbol.kind == TypeSymbolKind::Interface {
        resolving.remove(&symbol.canonical_name);
        return None;
    }
    let destructor = symbol
        .destructor
        .as_ref()
        .and_then(|destructor| resolved.semantic_db.definition_at(destructor.name_span));
    let mut fields = Vec::new();
    if symbol.kind == TypeSymbolKind::Struct {
        for (index, field) in symbol.fields.iter().enumerate() {
            let field_ty = substitute_type_expr_parameters(&field.ty, &substitutions);
            let Some(plan) = build_inner(&field_ty, fallback, sources, typed_hir, plans, resolving)
            else {
                continue;
            };
            fields.push(DropPlanField {
                index,
                ty: typed_hir.type_id(&field_ty)?,
                plan,
            });
        }
    }
    let mut variants = Vec::new();
    if symbol.kind == TypeSymbolKind::Enum {
        for variant in &symbol.variants {
            let definition = resolved.semantic_db.definition_at(variant.name_span)?;
            let mut payload_fields = Vec::new();
            for (index, payload) in variant.payload.iter().enumerate() {
                let payload_ty = substitute_type_expr_parameters(&payload.ty, &substitutions);
                let Some(plan) =
                    build_inner(&payload_ty, fallback, sources, typed_hir, plans, resolving)
                else {
                    continue;
                };
                payload_fields.push(DropPlanField {
                    index,
                    ty: typed_hir.type_id(&payload_ty)?,
                    plan,
                });
            }
            if !payload_fields.is_empty() {
                variants.push(DropPlanVariant {
                    definition,
                    fields: payload_fields,
                });
            }
        }
    }
    resolving.remove(&symbol.canonical_name);
    match (
        symbol.kind,
        destructor,
        fields.is_empty(),
        variants.is_empty(),
    ) {
        (TypeSymbolKind::Struct, destructor, false, _) => {
            push(plans, DropPlan::Struct { destructor, fields })
        }
        (TypeSymbolKind::Enum, None, _, false) => push(plans, DropPlan::Enum { variants }),
        (_, Some(destructor), true, _) => push(plans, DropPlan::Direct { destructor }),
        // Move semantics still require an ownership-consuming cleanup edge
        // even when no runtime destructor instruction is necessary.
        (_, None, true, true) => push(plans, DropPlan::Noop),
        _ => {
            let _ = definition;
            None
        }
    }
}

fn resolver_for<'a>(
    ty: &TypeExpr,
    fallback: &'a ResolveOutput,
    sources: &'a ResolvedSources<'a>,
) -> &'a ResolveOutput {
    sources.get(&ty.span().source).copied().unwrap_or(fallback)
}

fn type_symbol<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a crate::resolve::TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        name.rsplit_once('.')
            .and_then(|(_, short)| resolved.type_symbol_by_reference_name(short))
    })
}

fn push(plans: &mut Vec<DropPlan>, plan: DropPlan) -> Option<DropPlanId> {
    let id = DropPlanId::from_index(plans.len());
    plans.push(plan);
    Some(id)
}
