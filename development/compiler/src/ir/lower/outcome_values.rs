use super::context::LoweringContext;
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, ComposedOutcomeDestination, FallibleFailureMode, Instruction, Type,
};
use crate::outcomes::OutcomeLayer;

pub(super) fn lower_stored_fallible_expression(
    expression: &Expr,
    destination: ComposedOutcomeDestination,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let expression = expression.without_groups();
    let Expr::Identifier(identifier) = expression else {
        return Ok(None);
    };
    let Some(local) = context.outcome_local(&identifier.name) else {
        return Ok(None);
    };
    if local.storage.layers.len() != 1
        || local.storage.layers[0].layer != OutcomeLayer::Fallible
        || !payload_destination_matches(&local.payload_type, destination)
    {
        return Ok(None);
    }
    let layer = local.storage.layers[0];
    let tag_offset = checked_offset(layer.tag_offset, "tag")?;
    let error_offset = checked_offset(
        layer
            .failure_offset
            .expect("fallible layer has error storage"),
        "error",
    )?;
    let payload_offset = checked_offset(local.storage.payload_offset, "payload")?;
    Ok(Some(vec![Instruction::CheckStoredFallible {
        source: AggregateLocation::Slot(local.slot_index),
        tag_offset,
        error_offset,
        success_instructions: vec![Instruction::LoadStoredOutcomePayload {
            destination,
            source: AggregateLocation::Slot(local.slot_index),
            offset: payload_offset,
        }],
        failure_mode,
    }]))
}

fn checked_offset(offset: u64, role: &str) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(offset).map_err(|_| {
        vec![Diagnostic::error(
            "E8008",
            format!("stored outcome {role} offset exceeds u32"),
        )]
    })
}

fn payload_destination_matches(
    payload_type: &Type,
    destination: ComposedOutcomeDestination,
) -> bool {
    matches!(
        (payload_type, destination),
        (Type::I32, ComposedOutcomeDestination::I32(_))
            | (Type::U8, ComposedOutcomeDestination::U8(_))
            | (Type::Usize, ComposedOutcomeDestination::Usize(_))
            | (Type::Borrow { .. }, ComposedOutcomeDestination::Borrow(_))
            | (Type::Bool, ComposedOutcomeDestination::Bool(_))
            | (Type::Str, ComposedOutcomeDestination::Str(_))
            | (Type::Slice { .. }, ComposedOutcomeDestination::Slice(_))
    )
}
