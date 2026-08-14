//! Projection of semantic stored-outcome inspection into machine IR.
//!
//! MIR retains only the checked source type, outer outcome layer, and CFG
//! edges. Recursive tag, failure, and payload offsets are derived here from
//! the shared ABI outcome layout.

use super::{
    BackendContext, aggregate_location, invalid_mir_diagnostics, lower_branch_to_join,
    outcome_failure_mode,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{ComposedOutcomeDestination, Instruction};
use crate::mir::{LocalId, Operand, Place, ScalarType, ValueRepresentation};
use std::collections::HashSet;

pub(super) struct Inspection<'a> {
    pub(super) source: &'a Operand,
    pub(super) layer: crate::outcomes::OutcomeLayer,
    pub(super) destination: Place,
    pub(super) success: crate::mir::BasicBlockId,
    pub(super) failure: crate::mir::BasicBlockId,
    pub(super) failure_payload: Option<LocalId>,
    pub(super) visited: &'a mut HashSet<crate::mir::BasicBlockId>,
}

pub(super) fn lower(
    context: &BackendContext<'_>,
    inspection: Inspection<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let (Operand::Copy(source_place) | Operand::Move(source_place)) = inspection.source else {
        return Err(invalid_mir_diagnostics(
            "stored outcome inspection source is not a place",
        ));
    };
    let source_local = &context.body.locals[source_place.local.index()];
    let type_expr = context
        .typed_hir
        .type_expr_by_id(source_local.ty)
        .ok_or_else(|| invalid_mir_diagnostics("stored outcome source type is missing"))?;
    let shape =
        crate::outcomes::outcome_shape_with_resolver(type_expr, context.resolved, |source| {
            context.resolved_sources.get(&source).copied()
        });
    let Some(layer) = shape.layers.first() else {
        return Err(invalid_mir_diagnostics(
            "stored outcome inspection requires an outcome layer",
        ));
    };
    if *layer != inspection.layer {
        return Err(invalid_mir_diagnostics(
            "stored outcome inspection layer differs from its checked type",
        ));
    }
    let payload = crate::abi::abi_value_from_type_expr_with_resolver(
        &shape.payload,
        context.resolved,
        |source| context.resolved_sources.get(&source).copied(),
    )
    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
    let storage = shape.storage_layout(payload.layout).ok_or_else(|| {
        invalid_mir_diagnostics("stored outcome inspection has unsupported storage")
    })?;
    let source = aggregate_location(source_place, context)?;
    let success_instructions =
        outcome_success_projection(context, inspection.destination, source, &shape, &storage)?;
    let entry = &storage.layers[0];
    match inspection.layer {
        crate::outcomes::OutcomeLayer::Optional => Ok(Instruction::IfStoredOutcomeTag {
            source,
            tag_offset: entry.tag_offset as u32,
            success_instructions,
            outcome_instructions: lower_branch_to_join(
                context,
                inspection.failure,
                inspection.success,
                inspection.visited,
            )?,
        }),
        crate::outcomes::OutcomeLayer::Fallible => Ok(Instruction::CheckStoredFallible {
            source,
            tag_offset: entry.tag_offset as u32,
            error_offset: entry.failure_offset.ok_or_else(|| {
                invalid_mir_diagnostics("fallible stored outcome has no error storage")
            })? as u32,
            success_instructions,
            failure_mode: outcome_failure_mode(
                context,
                inspection.failure,
                inspection.success,
                inspection.failure_payload,
                inspection.visited,
            )?,
        }),
    }
}

pub(super) fn outcome_success_projection(
    context: &BackendContext<'_>,
    destination: Place,
    source: crate::ir::AggregateLocation,
    shape: &crate::outcomes::OutcomeShape,
    storage: &crate::outcomes::storage::OutcomeStorageLayout,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination_local = &context.body.locals[destination.local.index()];
    if destination_local.representation != ValueRepresentation::Aggregate {
        return Ok(vec![Instruction::LoadStoredOutcomePayload {
            destination: outcome_destination(context, destination)?,
            source,
            offset: storage.payload_offset as u32,
        }]);
    }

    let destination_value = super::aggregate_local_abi_value(destination_local.ty, context)?;
    let remaining_layout = if shape.layers.len() == 1 {
        storage.payload_layout
    } else {
        crate::outcomes::storage::outcome_storage_layout(&shape.layers[1..], storage.payload_layout)
            .layout
    };
    if destination_value.layout != remaining_layout {
        return Err(invalid_mir_diagnostics(
            "stored outcome success payload differs from its MIR destination",
        ));
    }
    Ok(vec![Instruction::CopyAggregateRange {
        destination: super::aggregate_location(&destination, context)?,
        destination_offset: 0,
        source,
        source_offset: storage.layers[0].success_offset as u32,
        layout: remaining_layout,
    }])
}

pub(super) fn lower_optional_loop_condition(
    context: &BackendContext<'_>,
    source: &Operand,
    destination: Place,
    mut success_instructions: Vec<Instruction>,
    mut failure_instructions: Vec<Instruction>,
) -> Result<(Instruction, crate::ir::BoolValue), Vec<Diagnostic>> {
    let (Operand::Copy(source_place) | Operand::Move(source_place)) = source else {
        return Err(invalid_mir_diagnostics(
            "optional loop inspection source is not a place",
        ));
    };
    let source_local = &context.body.locals[source_place.local.index()];
    let type_expr = context
        .typed_hir
        .type_expr_by_id(source_local.ty)
        .ok_or_else(|| invalid_mir_diagnostics("optional loop source type is missing"))?;
    let shape =
        crate::outcomes::outcome_shape_with_resolver(type_expr, context.resolved, |source| {
            context.resolved_sources.get(&source).copied()
        });
    if shape.layers.as_slice() != [crate::outcomes::OutcomeLayer::Optional] {
        return Err(invalid_mir_diagnostics(
            "optional loop source must contain exactly one optional layer",
        ));
    }
    let payload = crate::abi::abi_value_from_type_expr_with_resolver(
        &shape.payload,
        context.resolved,
        |source| context.resolved_sources.get(&source).copied(),
    )
    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
    let storage = shape
        .storage_layout(payload.layout)
        .ok_or_else(|| invalid_mir_diagnostics("optional loop storage is unsupported"))?;
    let source = aggregate_location(source_place, context)?;
    let condition =
        crate::ir::BoolLocation::Local(super::storage::machine_local_count(context.body));
    let mut projection =
        outcome_success_projection(context, destination, source, &shape, &storage)?;
    projection.append(&mut success_instructions);
    projection.push(Instruction::SetBool {
        destination: condition,
        value: crate::ir::BoolValue::Const(true),
    });
    failure_instructions.push(Instruction::SetBool {
        destination: condition,
        value: crate::ir::BoolValue::Const(false),
    });
    Ok((
        Instruction::IfStoredOutcomeTag {
            source,
            tag_offset: storage.layers[0].tag_offset as u32,
            success_instructions: projection,
            outcome_instructions: failure_instructions,
        },
        crate::ir::BoolValue::Location(condition),
    ))
}

fn outcome_destination(
    context: &BackendContext<'_>,
    destination: Place,
) -> Result<ComposedOutcomeDestination, Vec<Diagnostic>> {
    let representation = context.body.locals[destination.local.index()].representation;
    Ok(match representation {
        ValueRepresentation::Scalar(ScalarType::I32) => {
            ComposedOutcomeDestination::I32(super::i32_location(&destination, context)?)
        }
        ValueRepresentation::Scalar(ScalarType::U8) => {
            ComposedOutcomeDestination::U8(super::u8_location(&destination, context)?)
        }
        ValueRepresentation::Scalar(ScalarType::Usize) => {
            ComposedOutcomeDestination::Usize(super::usize_location(&destination, context)?)
        }
        ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
            ComposedOutcomeDestination::Integer {
                kind,
                destination: super::integer_location(&destination, kind, context)?,
            }
        }
        ValueRepresentation::Scalar(ScalarType::Bool) => {
            ComposedOutcomeDestination::Bool(super::bool_location(&destination, context)?)
        }
        ValueRepresentation::View(crate::mir::ViewKind::Str) => {
            ComposedOutcomeDestination::Str(super::str_location(&destination, context)?)
        }
        ValueRepresentation::View(crate::mir::ViewKind::Slice) => {
            ComposedOutcomeDestination::Slice(super::slice_location(&destination, context)?)
        }
        ValueRepresentation::Borrow => {
            ComposedOutcomeDestination::Borrow(super::usize_location(&destination, context)?)
        }
        ValueRepresentation::Unit | ValueRepresentation::Aggregate | ValueRepresentation::Error => {
            return Err(invalid_mir_diagnostics(
                "stored outcome payload representation is not yet projectable",
            ));
        }
    })
}

pub(super) fn lower_return(
    context: &BackendContext<'_>,
    source: &Operand,
) -> Result<Instruction, Vec<Diagnostic>> {
    let (Operand::Copy(source) | Operand::Move(source)) = source else {
        return Err(invalid_mir_diagnostics(
            "stored outcome return source is not a place",
        ));
    };
    if source.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected stored outcome returns are unsupported",
        ));
    }
    let local = &context.body.locals[source.local.index()];
    let type_expr = context
        .typed_hir
        .type_expr_by_id(local.ty)
        .ok_or_else(|| invalid_mir_diagnostics("stored outcome return type is missing"))?;
    let shape =
        crate::outcomes::outcome_shape_with_resolver(type_expr, context.resolved, |source| {
            context.resolved_sources.get(&source).copied()
        });
    let payload_abi = crate::abi::abi_value_from_type_expr_with_resolver(
        &shape.payload,
        context.resolved,
        |source| context.resolved_sources.get(&source).copied(),
    )
    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
    let storage = shape
        .storage_layout(payload_abi.layout)
        .ok_or_else(|| invalid_mir_diagnostics("stored outcome return has unsupported storage"))?;
    let payload_type = super::super::types::return_type_from_type_expr_with_resolver(
        &shape.payload,
        context.resolved,
        |source| context.resolved_sources.get(&source).copied(),
    )
    .ok_or_else(|| invalid_mir_diagnostics("stored outcome return payload is unsupported"))?;
    let expected = context.return_type;
    let expected_layers = match expected {
        crate::ir::Type::Optional(_) => vec![crate::outcomes::OutcomeLayer::Optional],
        crate::ir::Type::Fallible(_) => vec![crate::outcomes::OutcomeLayer::Fallible],
        crate::ir::Type::ComposedOutcome { outer, inner, .. } => vec![*outer, *inner],
        _ => {
            return Err(invalid_mir_diagnostics(
                "stored outcome MIR return belongs to a plain callable",
            ));
        }
    };
    if shape.layers != expected_layers || expected.success_type() != &payload_type {
        return Err(invalid_mir_diagnostics(
            "stored outcome MIR return type differs from the callable result",
        ));
    }
    Ok(Instruction::ReturnStoredOutcome {
        source: aggregate_location(source, context)?,
        storage,
        payload_type,
    })
}
