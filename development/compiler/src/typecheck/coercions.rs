//! Selection of one-step, expected-type-directed borrow coercions.
//!
//! This module deliberately does not widen ordinary type assignability. A
//! coercion is selected only for an expression whose computed type is already
//! a borrow of the nominal source type and only when its declared target is
//! the concrete expected type.

use super::model::{Type, TypeEnvironment};
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::MethodReceiverMode;
use crate::resolve::{CoercionSignature, ResolveOutput, TypeSymbol};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedCoercion {
    pub(super) def_id: Option<crate::semantic::DefId>,
    pub(super) declaration_span: crate::source::ByteSpan,
    pub(super) focus_span: crate::source::ByteSpan,
    pub(super) receiver_mode: MethodReceiverMode,
    pub(super) source_is_readwrite: bool,
    pub(super) source_type: Type,
    pub(super) target_type: Type,
    pub(super) substitutions: HashMap<String, Type>,
    pub(super) has_explicit_result_provenance: bool,
    pub(super) requirement_span: Option<crate::source::ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoercionRejection {
    MissingSourceBorrow,
    RequiresReadwriteBorrow,
    Inaccessible,
}

pub(super) fn select_coercion(
    expected: &Type,
    actual: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<SelectedCoercion> {
    if expected.is_unknown_or_unresolved() || actual.is_unknown_or_unresolved() {
        return None;
    }
    let Type::Borrow {
        is_readwrite,
        inner: source_type,
    } = actual
    else {
        return None;
    };
    let mut candidates = source_type
        .nominal_name()
        .and_then(|source_name| resolved.type_symbol_by_reference_name(source_name))
        .and_then(|symbol| source_substitutions(symbol, source_type).map(|values| (symbol, values)))
        .map(|(symbol, substitutions)| {
            symbol
                .coercions
                .iter()
                .filter(|coercion| coercion.is_accessible)
                .filter(|coercion| receiver_accepts(coercion.receiver.mode, *is_readwrite))
                .filter_map(|coercion| {
                    selected_candidate(
                        coercion,
                        source_type,
                        expected,
                        resolved,
                        &substitutions,
                        *is_readwrite,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(requirement) = environment.coercion_requirement(actual, expected) {
        candidates.push(selected_requirement(
            requirement,
            source_type,
            expected,
            *is_readwrite,
        ));
    }

    // A readwrite borrow can be weakened for a readonly receiver. Prefer the
    // exact readwrite contract when both declarations expose the same target.
    candidates.sort_by_key(|candidate| {
        if candidate.receiver_mode == MethodReceiverMode::ReadwriteBorrow {
            0
        } else {
            1
        }
    });
    candidates.into_iter().next()
}

pub(super) fn receiver_coercion_candidates(
    source_type: &Type,
    source_is_readwrite: bool,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Vec<SelectedCoercion> {
    let mut candidates = source_type
        .nominal_name()
        .and_then(|source_name| resolved.type_symbol_by_reference_name(source_name))
        .and_then(|symbol| source_substitutions(symbol, source_type).map(|values| (symbol, values)))
        .map(|(symbol, substitutions)| {
            symbol
                .coercions
                .iter()
                .filter(|coercion| coercion.is_accessible)
                .filter(|coercion| receiver_accepts(coercion.receiver.mode, source_is_readwrite))
                .map(|coercion| {
                    let target_type = type_expr_to_type_with_substitutions(
                        &coercion.target,
                        resolved,
                        Some(source_type),
                        &substitutions,
                    );
                    SelectedCoercion {
                        def_id: Some(coercion.def_id),
                        declaration_span: coercion.declaration_span,
                        focus_span: coercion.focus_span,
                        receiver_mode: coercion.receiver.mode,
                        source_is_readwrite,
                        source_type: source_type.clone(),
                        target_type,
                        substitutions: substitutions.clone(),
                        has_explicit_result_provenance: coercion.result_provenance.is_some(),
                        requirement_span: None,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let actual = Type::Borrow {
        is_readwrite: source_is_readwrite,
        inner: Box::new(source_type.clone()),
    };
    candidates.extend(
        environment
            .coercion_requirements_for_source(&actual)
            .map(|requirement| {
                selected_requirement(
                    requirement,
                    source_type,
                    &requirement.target,
                    source_is_readwrite,
                )
            }),
    );
    candidates.sort_by_key(|candidate| {
        if candidate.receiver_mode == MethodReceiverMode::ReadwriteBorrow {
            0
        } else {
            1
        }
    });
    candidates
}

pub(super) fn coercion_rejection(
    expected: &Type,
    actual: &Type,
    resolved: &ResolveOutput,
) -> Option<CoercionRejection> {
    let (source_type, source_is_readwrite, source_is_borrowed) = match actual {
        Type::Borrow {
            is_readwrite,
            inner,
        } => (inner.as_ref(), *is_readwrite, true),
        actual if actual.nominal_name().is_some() => (actual, false, false),
        _ => return None,
    };
    let source_name = source_type.nominal_name()?;
    let symbol = resolved.type_symbol_by_reference_name(source_name)?;
    let substitutions = source_substitutions(symbol, source_type)?;
    let matching = symbol
        .coercions
        .iter()
        .filter(|coercion| {
            coercion_target_matches(coercion, source_type, expected, resolved, &substitutions)
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }
    if !source_is_borrowed {
        return Some(CoercionRejection::MissingSourceBorrow);
    }
    if matching.iter().all(|coercion| !coercion.is_accessible) {
        return Some(CoercionRejection::Inaccessible);
    }
    if !source_is_readwrite
        && matching
            .iter()
            .filter(|coercion| coercion.is_accessible)
            .all(|coercion| coercion.receiver.mode == MethodReceiverMode::ReadwriteBorrow)
    {
        return Some(CoercionRejection::RequiresReadwriteBorrow);
    }
    None
}

fn selected_candidate(
    coercion: &CoercionSignature,
    source_type: &Type,
    expected: &Type,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
    source_is_readwrite: bool,
) -> Option<SelectedCoercion> {
    let target_type = type_expr_to_type_with_substitutions(
        &coercion.target,
        resolved,
        Some(source_type),
        substitutions,
    );
    (target_type == *expected).then(|| SelectedCoercion {
        def_id: Some(coercion.def_id),
        declaration_span: coercion.declaration_span,
        focus_span: coercion.focus_span,
        receiver_mode: coercion.receiver.mode,
        source_is_readwrite,
        source_type: source_type.clone(),
        target_type,
        substitutions: substitutions.clone(),
        has_explicit_result_provenance: coercion.result_provenance.is_some(),
        requirement_span: None,
    })
}

fn selected_requirement(
    requirement: &super::model::CoercionRequirement,
    source_type: &Type,
    target_type: &Type,
    source_is_readwrite: bool,
) -> SelectedCoercion {
    let receiver_mode = match requirement.source {
        Type::Borrow {
            is_readwrite: true, ..
        } => MethodReceiverMode::ReadwriteBorrow,
        _ => MethodReceiverMode::ReadonlyBorrow,
    };
    SelectedCoercion {
        def_id: None,
        declaration_span: requirement.as_span,
        focus_span: requirement.as_span,
        receiver_mode,
        source_is_readwrite,
        source_type: source_type.clone(),
        target_type: target_type.clone(),
        substitutions: HashMap::new(),
        has_explicit_result_provenance: false,
        requirement_span: Some(requirement.as_span),
    }
}

fn coercion_target_matches(
    coercion: &CoercionSignature,
    source_type: &Type,
    expected: &Type,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
) -> bool {
    type_expr_to_type_with_substitutions(
        &coercion.target,
        resolved,
        Some(source_type),
        substitutions,
    ) == *expected
}

fn receiver_accepts(receiver: MethodReceiverMode, actual_is_readwrite: bool) -> bool {
    match receiver {
        MethodReceiverMode::ReadonlyBorrow => true,
        MethodReceiverMode::ReadwriteBorrow => actual_is_readwrite,
        MethodReceiverMode::Owned => false,
    }
}

fn source_substitutions(symbol: &TypeSymbol, source: &Type) -> Option<HashMap<String, Type>> {
    let arguments = match source {
        Type::Named(name) if *name == symbol.canonical_name => &[][..],
        Type::Generic { name, arguments } if *name == symbol.canonical_name => arguments.as_slice(),
        _ => return None,
    };
    (arguments.len() == symbol.generic_parameters.len()).then(|| {
        symbol
            .generic_parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect()
    })
}

pub(crate) fn specialize_coercion_plan(
    mut plan: super::facts::TypecheckCoercionPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckCoercionPlan> {
    plan.requirement_span?;
    let source_type = super::type_expr::type_expr_to_type(&plan.self_ty, resolved);
    let target_type = super::type_expr::type_expr_to_type(&plan.target_ty, resolved);
    let actual = Type::Borrow {
        is_readwrite: plan.source_is_readwrite,
        inner: Box::new(source_type),
    };
    let selected = select_coercion(&target_type, &actual, resolved, &TypeEnvironment::default())?;
    if selected.requirement_span.is_some() {
        return None;
    }
    plan.declaration_span = selected.declaration_span;
    plan.def_id = selected.def_id;
    plan.focus_span = selected.focus_span;
    plan.receiver_mode = selected.receiver_mode;
    plan.target_name = format!(
        "{}.__nocter$coerce${}",
        crate::ast::canonical_type_expr(&plan.self_ty),
        selected.focus_span.start
    );
    plan.substitutions = selected
        .substitutions
        .into_iter()
        .map(|(name, ty)| {
            let mut free = std::collections::HashSet::new();
            super::facts::type_to_type_expr_allowing_parameters(&ty, plan.self_ty.span(), &mut free)
                .map(|ty| (name, ty))
        })
        .collect::<Option<HashMap<_, _>>>()?;
    plan.has_explicit_result_provenance = selected.has_explicit_result_provenance;
    plan.requirement_span = None;
    Some(plan)
}

pub(crate) fn specialize_coercion_plan_across_resolvers<'a>(
    plan: super::facts::TypecheckCoercionPlan,
    resolvers: impl IntoIterator<Item = &'a ResolveOutput>,
) -> Option<super::facts::TypecheckCoercionPlan> {
    if plan.requirement_span.is_none() {
        return Some(plan);
    }
    resolvers
        .into_iter()
        .find_map(|resolved| specialize_coercion_plan(plan.clone(), resolved))
}
