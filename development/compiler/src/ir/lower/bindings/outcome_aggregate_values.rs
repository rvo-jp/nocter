use super::*;

pub(super) fn lower_stored_outcome_aggregate_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (identifier, mode) = match unwrap_group(&statement.initializer) {
        Expr::Force(force) => {
            let Expr::Identifier(identifier) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            (identifier, StoredAggregateConsumer::Force)
        }
        Expr::Propagate(propagation) => {
            let Expr::Identifier(identifier) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            (identifier, StoredAggregateConsumer::Propagate)
        }
        Expr::Catch(catch) => {
            let Expr::Identifier(identifier) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            (identifier, StoredAggregateConsumer::Catch(catch))
        }
        Expr::Otherwise(otherwise) => {
            let Expr::Identifier(identifier) = unwrap_group(&otherwise.value) else {
                return Ok(None);
            };
            (
                identifier,
                StoredAggregateConsumer::Otherwise(&otherwise.fallback),
            )
        }
        _ => return Ok(None),
    };
    let Some(local) = context.outcome_local(&identifier.name) else {
        return Ok(None);
    };
    if !local.is_live
        || local.storage.layers.len() != 1
        || !matches!(
            local.payload_type,
            Type::Aggregate { .. } | Type::DirectAggregate { .. }
        )
    {
        return Ok(None);
    }
    let layer = local.storage.layers[0];
    if (mode.requires_fallible() && layer.layer != OutcomeLayer::Fallible)
        || (matches!(mode, StoredAggregateConsumer::Otherwise(_))
            && layer.layer != OutcomeLayer::Optional)
    {
        return Ok(None);
    }
    let Some(operand_ty) = context.expression_type_expr(identifier.span) else {
        return Ok(None);
    };
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };
    let shape = outcome_shape_with_resolver(&operand_ty, resolved, |source| {
        context.resolved_source(source)
    });
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &shape.payload,
        root_source,
        resolved,
        |source| context.resolved_source(source),
    )
    .unwrap_or_default();
    let is_copy =
        type_expr_is_copy_aggregate_value_with_resolver(&shape.payload, resolved, |source| {
            context.resolved_source(source)
        });
    let drop_kind = context.aggregate_drop_for_type_expr(&shape.payload);
    let destination_slot = context.define_aggregate_local(
        statement.name.clone(),
        local.storage.payload_layout,
        is_copy,
        drop_kind,
        fields,
    );
    let copy = Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(destination_slot),
        destination_offset: 0,
        source: AggregateLocation::Slot(local.slot_index),
        source_offset: checked_outcome_offset(local.storage.payload_offset, "payload")?,
        layout: local.storage.payload_layout,
    };
    let guarded = match mode {
        StoredAggregateConsumer::Force if layer.layer == OutcomeLayer::Optional => {
            Instruction::IfStoredOutcomeTag {
                source: AggregateLocation::Slot(local.slot_index),
                tag_offset: checked_outcome_offset(layer.tag_offset, "tag")?,
                success_instructions: vec![copy],
                outcome_instructions: vec![Instruction::Trap],
            }
        }
        StoredAggregateConsumer::Force => {
            stored_fallible_aggregate_check(&local, layer, copy, OutcomeFailureMode::Trap)?
        }
        StoredAggregateConsumer::Propagate if layer.layer == OutcomeLayer::Optional => {
            Instruction::IfStoredOutcomeTag {
                source: AggregateLocation::Slot(local.slot_index),
                tag_offset: checked_outcome_offset(layer.tag_offset, "tag")?,
                success_instructions: vec![copy],
                outcome_instructions: stored_optional_propagation_instructions(context)?,
            }
        }
        StoredAggregateConsumer::Propagate => stored_fallible_aggregate_check(
            &local,
            layer,
            copy,
            propagating_outcome_mode_for_layer(layer.layer, context)?,
        )?,
        StoredAggregateConsumer::Catch(catch) => stored_fallible_aggregate_check(
            &local,
            layer,
            copy,
            lower_catch_failure_mode(catch, context, 0)?,
        )?,
        StoredAggregateConsumer::Otherwise(fallback) => {
            let failure = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                context,
                None,
                |expression, fallback_context| {
                    lower_aggregate_return_expression_to_location(
                        expression,
                        &local.payload_type,
                        AggregateLocation::Slot(destination_slot),
                        fallback_context.function_name(),
                        resolved,
                        fallback_context,
                    )
                },
                "stored aggregate `otherwise` fallback must produce the payload type or exit",
            )?;
            let outcome_instructions = match failure {
                OutcomeFailureMode::Handle { instructions }
                | OutcomeFailureMode::Recover { instructions } => instructions,
                _ => unreachable!("otherwise fallback produces handle or recover mode"),
            };
            Instruction::IfStoredOutcomeTag {
                source: AggregateLocation::Slot(local.slot_index),
                tag_offset: checked_outcome_offset(layer.tag_offset, "tag")?,
                success_instructions: vec![copy],
                outcome_instructions,
            }
        }
    };
    if !is_copy {
        context.mark_outcome_local_moved(&identifier.name);
    }
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot {
            slot_index: destination_slot,
            layout: local.storage.payload_layout,
        },
        guarded,
    ]))
}

fn stored_fallible_aggregate_check(
    local: &OutcomeLocal,
    layer: crate::outcomes::storage::OutcomeLayerStorage,
    copy: Instruction,
    failure_mode: OutcomeFailureMode,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(Instruction::CheckStoredFallible {
        source: AggregateLocation::Slot(local.slot_index),
        tag_offset: checked_outcome_offset(layer.tag_offset, "tag")?,
        error_offset: checked_outcome_offset(
            layer
                .failure_offset
                .expect("fallible layer has error storage"),
            "error",
        )?,
        success_instructions: vec![copy],
        failure_mode,
    })
}

fn checked_outcome_offset(offset: u64, role: &str) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(offset).map_err(|_| {
        vec![Diagnostic::error(
            "E8008",
            format!("stored outcome {role} offset exceeds u32"),
        )]
    })
}

enum StoredAggregateConsumer<'a> {
    Force,
    Propagate,
    Catch(&'a crate::ast::CatchExpr),
    Otherwise(&'a Block),
}

impl StoredAggregateConsumer<'_> {
    fn requires_fallible(&self) -> bool {
        matches!(self, Self::Catch(_))
    }
}
