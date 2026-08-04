use super::bindings::lower_otherwise_recover_or_handle_failure_mode;
use super::context::LoweringContext;
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, ComposedOutcomeDestination, FallibleFailureMode, Instruction, Type,
};
use crate::outcomes::OutcomeLayer;

pub(super) fn lower_stored_outcome_expression(
    expression: &Expr,
    destination: ComposedOutcomeDestination,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let expression = expression.without_groups();
    if let Expr::Otherwise(otherwise) = expression
        && let Expr::Identifier(identifier) = otherwise.value.without_groups()
        && let Some(local) = context.outcome_local(&identifier.name)
        && local.storage.layers.len() == 2
        && local.storage.layers[0].layer == OutcomeLayer::Optional
        && local.storage.layers[1].layer == OutcomeLayer::Fallible
        && payload_destination_matches(&local.payload_type, destination)
    {
        let fallback = lower_otherwise_recover_or_handle_failure_mode(
            &otherwise.fallback,
            context,
            None,
            |_expression, _context| {
                Err(vec![Diagnostic::error(
                    "E8008",
                    "stored optional-fallible fallback must exit the current control path",
                )])
            },
            "stored optional-fallible fallback must exit the current control path",
        )?;
        let outcome_instructions = match fallback {
            FallibleFailureMode::Handle { instructions }
            | FallibleFailureMode::Recover { instructions } => instructions,
            _ => unreachable!("otherwise fallback produces handle or recover mode"),
        };
        let outer = local.storage.layers[0];
        let inner = local.storage.layers[1];
        return Ok(Some(vec![Instruction::IfStoredOutcomeTag {
            source: AggregateLocation::Slot(local.slot_index),
            tag_offset: checked_offset(outer.tag_offset, "outer tag")?,
            success_instructions: vec![Instruction::CheckStoredFallible {
                source: AggregateLocation::Slot(local.slot_index),
                tag_offset: checked_offset(inner.tag_offset, "inner tag")?,
                error_offset: checked_offset(
                    inner
                        .failure_offset
                        .expect("fallible layer has error storage"),
                    "inner error",
                )?,
                success_instructions: vec![Instruction::LoadStoredOutcomePayload {
                    destination,
                    source: AggregateLocation::Slot(local.slot_index),
                    offset: checked_offset(local.storage.payload_offset, "payload")?,
                }],
                failure_mode,
            }],
            outcome_instructions,
        }]));
    }
    let Expr::Identifier(identifier) = expression else {
        return Ok(None);
    };
    let Some(local) = context.outcome_local(&identifier.name) else {
        return Ok(None);
    };
    if local.storage.layers.len() != 1
        || !payload_destination_matches(&local.payload_type, destination)
    {
        return Ok(None);
    }
    let layer = local.storage.layers[0];
    let tag_offset = checked_offset(layer.tag_offset, "tag")?;
    let payload_offset = checked_offset(local.storage.payload_offset, "payload")?;
    let success_instructions = vec![Instruction::LoadStoredOutcomePayload {
        destination,
        source: AggregateLocation::Slot(local.slot_index),
        offset: payload_offset,
    }];
    match layer.layer {
        OutcomeLayer::Fallible => Ok(Some(vec![Instruction::CheckStoredFallible {
            source: AggregateLocation::Slot(local.slot_index),
            tag_offset,
            error_offset: checked_offset(
                layer
                    .failure_offset
                    .expect("fallible layer has error storage"),
                "error",
            )?,
            success_instructions,
            failure_mode,
        }])),
        OutcomeLayer::Optional => Ok(Some(vec![Instruction::IfStoredOutcomeTag {
            source: AggregateLocation::Slot(local.slot_index),
            tag_offset,
            success_instructions,
            outcome_instructions: optional_outcome_instructions(failure_mode)?,
        }])),
    }
}

fn optional_outcome_instructions(
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match failure_mode {
        FallibleFailureMode::Propagate => Ok(vec![Instruction::ReturnOptionalNone]),
        FallibleFailureMode::Trap => Ok(vec![Instruction::Trap]),
        FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions } => Ok(instructions),
        FallibleFailureMode::PropagateWithCleanup { .. } | FallibleFailureMode::Catch { .. } => {
            Err(vec![Diagnostic::error(
                "E8008",
                "optional outcome received a fallible-only failure mode",
            )])
        }
    }
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
