use super::*;

pub(in crate::ir::lower) fn lower_outcome_field_to_location(
    storage: &crate::outcomes::storage::OutcomeStorageLayout,
    expression: &Expr,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            if let Some(local) = context.outcome_local(&identifier.name) {
                if !local.is_live || local.storage != *storage || !local.is_copy {
                    return Ok(None);
                }
                return Ok(Some(vec![Instruction::CopyAggregateRange {
                    destination,
                    destination_offset,
                    source: AggregateLocation::Slot(local.slot_index),
                    source_offset: 0,
                    layout: storage.layout,
                }]));
            }
        }
        Expr::Call(call) => {
            let Some(return_type) = context
                .expression_type_expr(call.span)
                .or_else(|| context.call_return_type_expr(call))
            else {
                return Ok(None);
            };
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Ok(None);
            };
            let shape = outcome_shape_with_resolver(&return_type, resolved, |source| {
                context.resolved_source(source)
            });
            if shape.layers.is_empty() || !shape.is_supported_callable_shape() {
                return Ok(None);
            }
            let payload_abi =
                abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok();
            let Some(storage) = payload_abi
                .and_then(|payload| shape.storage_layout(payload.layout))
                .filter(|actual| actual == storage)
            else {
                return Ok(None);
            };
            let Some(payload_type) =
                return_type_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
            else {
                return Ok(None);
            };
            let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            let storage_layout = storage.layout;
            let source_slot = temporaries.next_aggregate_slot();
            let (mut instructions, arguments) =
                lower_call_arguments_to_scalar_arguments_with_temporaries(
                    call,
                    &target,
                    &call_name,
                    context,
                    temporaries,
                )?;
            instructions.push(Instruction::ReserveAggregateSlot {
                slot_index: source_slot,
                layout: storage_layout,
            });
            instructions.push(Instruction::CallStoredOutcome {
                destination: AggregateLocation::Slot(source_slot),
                target,
                arguments,
                storage,
                payload_type,
            });
            instructions.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source_slot),
                source_offset: 0,
                layout: storage_layout,
            });
            return Ok(Some(instructions));
        }
        _ => {}
    }

    if matches!(expression.without_groups(), Expr::NoneLiteral(_)) {
        let Some(layer) = storage
            .layers
            .first()
            .filter(|layer| layer.layer == crate::outcomes::OutcomeLayer::Optional)
        else {
            return Ok(None);
        };
        let offset = destination_offset
            .checked_add(u32::try_from(layer.tag_offset).map_err(|_| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?)
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        return Ok(Some(vec![Instruction::StoreAggregateUsize {
            destination,
            offset,
            value: UsizeValue::Const(1),
        }]));
    }

    let Some(expression_type) = context.expression_type_expr(expression.span()) else {
        return Ok(None);
    };
    let Some(payload) = context.abi_value_for_type_expr(&expression_type) else {
        return Ok(None);
    };
    if matches!(payload.ty, AbiType::Outcome { .. }) || payload.layout != storage.payload_layout {
        return Ok(None);
    }
    let mut instructions = Vec::with_capacity(storage.layers.len() + 1);
    for layer in &storage.layers {
        let offset = destination_offset
            .checked_add(u32::try_from(layer.tag_offset).map_err(|_| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?)
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        instructions.push(Instruction::StoreAggregateUsize {
            destination,
            offset,
            value: UsizeValue::Const(0),
        });
    }
    let payload_offset = destination_offset
        .checked_add(u32::try_from(storage.payload_offset).map_err(|_| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?)
        .ok_or_else(|| unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject))?;
    instructions.extend(lower_aggregate_field_to_location(
        &payload.ty,
        expression,
        destination,
        payload_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
    )?);
    Ok(Some(instructions))
}
