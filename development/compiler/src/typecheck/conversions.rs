//! Semantic selection shared by contextual compatibility and explicit `as`.
//!
//! This module classifies a conversion once. Consumers decide whether a
//! classified result needs a persisted typecheck plan; they do not repeat
//! coercion lookup or infer conversion meaning from source spelling.

use super::coercions::{CoercionRejection, SelectedCoercion};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use crate::ast::Expr;
use crate::resolve::ResolveOutput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConversionMode {
    Contextual,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SelectedConversionKind {
    Exact,
    LosslessInteger,
    CapabilityWeakening,
    BorrowCoercion(SelectedCoercion),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedConversion {
    pub(super) source_type: Type,
    pub(super) target_type: Type,
    pub(super) kind: SelectedConversionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConversionRejection {
    MissingSourceBorrow,
    RequiresReadwriteBorrow,
    InaccessibleCoercion,
    Unsupported,
}

pub(super) fn select_expression_conversion(
    mode: ConversionMode,
    target: &Type,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SelectedConversion, ConversionRejection> {
    let source = expression_type(expression, resolved, environment);
    select_conversion(mode, target, &source, expression, resolved, environment)
}

fn select_conversion(
    mode: ConversionMode,
    target: &Type,
    source: &Type,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SelectedConversion, ConversionRejection> {
    if source.is_unknown_or_unresolved() || target.is_unknown_or_unresolved() {
        return Err(ConversionRejection::Unsupported);
    }
    if mode == ConversionMode::Contextual && super::operations::is_assignable(target, source) {
        return Ok(selected(source, target, SelectedConversionKind::Exact));
    }
    if borrow_capability_can_weaken(source, target) {
        return Ok(selected(
            source,
            target,
            SelectedConversionKind::CapabilityWeakening,
        ));
    }
    if mode == ConversionMode::Explicit
        && super::operations::is_lossless_integer_conversion(
            source,
            expression,
            target,
            resolved,
            environment,
        )
    {
        return Ok(selected(
            source,
            target,
            SelectedConversionKind::LosslessInteger,
        ));
    }
    if let Some(coercion) = super::coercions::select_coercion(target, source, resolved) {
        return Ok(selected(
            source,
            target,
            SelectedConversionKind::BorrowCoercion(coercion),
        ));
    }

    Err(
        match super::coercions::coercion_rejection(target, source, resolved) {
            Some(CoercionRejection::MissingSourceBorrow) => {
                ConversionRejection::MissingSourceBorrow
            }
            Some(CoercionRejection::RequiresReadwriteBorrow) => {
                ConversionRejection::RequiresReadwriteBorrow
            }
            Some(CoercionRejection::Inaccessible) => ConversionRejection::InaccessibleCoercion,
            None => ConversionRejection::Unsupported,
        },
    )
}

fn selected(source: &Type, target: &Type, kind: SelectedConversionKind) -> SelectedConversion {
    SelectedConversion {
        source_type: source.clone(),
        target_type: target.clone(),
        kind,
    }
}

pub(super) fn selected_receiver_coercion(
    source: &Type,
    coercion: super::coercions::SelectedCoercion,
) -> SelectedConversion {
    SelectedConversion {
        source_type: source.clone(),
        target_type: coercion.target_type.clone(),
        kind: SelectedConversionKind::BorrowCoercion(coercion),
    }
}

fn borrow_capability_can_weaken(source: &Type, target: &Type) -> bool {
    matches!(
        (source, target),
        (
            Type::Borrow {
                is_readwrite: true,
                inner: source,
            },
            Type::Borrow {
                is_readwrite: false,
                inner: target,
            },
        ) if source == target
    )
}
