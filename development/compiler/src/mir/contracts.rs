//! Shared native contracts projected from checked source types.
//!
//! MIR construction, buildability, and machine-IR projection must agree on
//! one return representation.  This module is the only boundary that maps a
//! resolved callable result type onto MIR's logical representation and
//! outcome mode; ABI layout remains in `abi` and physical storage remains in
//! machine-IR lowering.

use crate::abi::{AbiType, AbiTypeContract};
use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, ResolvedSources};
use crate::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallableReturnContract {
    pub(crate) representation: super::ValueRepresentation,
    pub(crate) mode: super::ReturnMode,
    pub(crate) outcome_layers: Vec<crate::outcomes::OutcomeLayer>,
}

pub(crate) fn callable_return_contract(
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<CallableReturnContract> {
    callable_return_contract_with_resolver(return_type, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
}

fn callable_return_contract_with_resolver<'a, F>(
    return_type: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: F,
) -> Option<CallableReturnContract>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let shape = crate::outcomes::outcome_shape_with_resolver(return_type, resolved, &resolver);
    let mode = if shape
        .layers
        .contains(&crate::outcomes::OutcomeLayer::Fallible)
    {
        super::ReturnMode::Fallible
    } else {
        super::ReturnMode::Plain
    };
    let contract = crate::abi::abi_type_contract_from_type_expr_with_resolver(
        &shape.payload,
        resolved,
        resolver,
    )
    .ok()?;
    let representation = value_representation(&contract)?;
    Some(CallableReturnContract {
        representation,
        mode,
        outcome_layers: shape.layers,
    })
}

pub(crate) fn value_representation(
    contract: &AbiTypeContract,
) -> Option<super::ValueRepresentation> {
    Some(match contract {
        AbiTypeContract::Void | AbiTypeContract::Never => super::ValueRepresentation::Unit,
        AbiTypeContract::Error => super::ValueRepresentation::Error,
        AbiTypeContract::Value(value) => match &value.ty {
            AbiType::Bool => super::ValueRepresentation::Scalar(super::ScalarType::Bool),
            AbiType::Pointer => super::ValueRepresentation::Scalar(super::ScalarType::Usize),
            AbiType::StrView => super::ValueRepresentation::View(super::ViewKind::Str),
            AbiType::SliceView => super::ValueRepresentation::View(super::ViewKind::Slice),
            AbiType::Borrow => super::ValueRepresentation::Borrow,
            AbiType::Array { .. }
            | AbiType::Struct(_)
            | AbiType::Enum(_)
            | AbiType::Outcome { .. } => super::ValueRepresentation::Aggregate,
            ty => super::ValueRepresentation::Scalar(scalar_type(ty)?),
        },
    })
}

pub(crate) fn scalar_type(ty: &AbiType) -> Option<super::ScalarType> {
    let kind = ty.integer_type()?;
    Some(match kind {
        crate::integer::IntegerType::I32 => super::ScalarType::I32,
        crate::integer::IntegerType::U8 => super::ScalarType::U8,
        crate::integer::IntegerType::Usize => super::ScalarType::Usize,
        kind => super::ScalarType::Integer(kind),
    })
}
