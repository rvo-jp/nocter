//! Selection of one-step, expected-type-directed borrow coercions.
//!
//! This module deliberately does not widen ordinary type assignability. A
//! coercion is selected only for an expression whose computed type is already
//! a borrow of the nominal source type and only when its declared target is
//! the concrete expected type.

use super::model::Type;
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::MethodReceiverMode;
use crate::resolve::{CoercionSignature, ResolveOutput, TypeSymbol};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedCoercion {
    pub(super) declaration_span: crate::source::ByteSpan,
    pub(super) focus_span: crate::source::ByteSpan,
    pub(super) receiver_mode: MethodReceiverMode,
    pub(super) source_is_readwrite: bool,
    pub(super) source_type: Type,
    pub(super) target_type: Type,
    pub(super) substitutions: HashMap<String, Type>,
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
    let source_name = source_type.nominal_name()?;
    let symbol = resolved.type_symbol_by_reference_name(source_name)?;
    let substitutions = source_substitutions(symbol, source_type)?;

    let mut candidates = symbol
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
        .collect::<Vec<_>>();

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
        declaration_span: coercion.declaration_span,
        focus_span: coercion.focus_span,
        receiver_mode: coercion.receiver.mode,
        source_is_readwrite,
        source_type: source_type.clone(),
        target_type,
        substitutions: substitutions.clone(),
    })
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
