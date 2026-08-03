use super::*;

pub(in crate::ir::lower) fn lower_outcome_field_to_location(
    expected_layout: ValueLayout,
    expression: &Expr,
    destination: AggregateLocation,
    destination_offset: u32,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            let Some(local) = context.outcome_local(&identifier.name) else {
                return Ok(None);
            };
            if !local.is_live || local.storage.layout != expected_layout || !local.is_copy {
                return Ok(None);
            }
            Ok(Some(vec![Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(local.slot_index),
                source_offset: 0,
                layout: expected_layout,
            }]))
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
                .filter(|storage| storage.layout == expected_layout)
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
                layout: expected_layout,
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
                layout: expected_layout,
            });
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
}
