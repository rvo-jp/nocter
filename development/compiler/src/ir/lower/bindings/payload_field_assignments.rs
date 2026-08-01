use super::*;

/// Constructs a replacement payload enum in temporary storage before touching
/// the existing field. The temporary obligation lets propagation unwind only
/// the payload prefix that has actually finished initialization.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_payload_enum_field_replacement(
    target: &MemberExpr,
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    drop_kind: Option<&AggregateDrop>,
    expression: &Expr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if payload_enum_constructor_member_and_arguments(expression).is_none() {
        return Ok(None);
    }
    let Some(target_type) = context.expression_type_expr(target.span) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let value = abi_value_from_type_expr_with_resolver(&target_type, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_assignment_diagnostic())?;
    let AbiType::Enum(enum_) = &value.ty else {
        return Err(unsupported_assignment_diagnostic());
    };
    if value.layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let replacement_slot = context.reserve_aggregate_slot_index();
    let progress = match drop_kind {
        Some(drop_kind @ AggregateDrop::PayloadEnum(_)) => {
            let progress = PayloadInitializationProgress::with_allocator(
                expression, enum_, drop_kind, context,
            )?;
            if !context.register_temporary_payload_fields_drop(
                replacement_slot,
                layout,
                drop_kind.clone(),
                progress.tag(),
                progress.drop_states(),
            ) {
                return Err(unsupported_assignment_diagnostic());
            }
            Some(progress)
        }
        None => None,
        Some(_) => return Err(unsupported_assignment_diagnostic()),
    };

    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    if let Some(progress) = &progress {
        instructions.extend(progress.initialize());
    }
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_payload_enum_constructor_to_location_with_progress(
        expression,
        &value.ty,
        layout,
        AggregateLocation::Slot(replacement_slot),
        "E8008",
        "assignments",
        resolved,
        context,
        &mut temporaries,
        progress.as_ref(),
    );
    if progress.is_some() {
        context.release_temporary_aggregate_drop(replacement_slot);
    }
    let Some(mut constructor) = lowered? else {
        return Err(unsupported_assignment_diagnostic());
    };
    instructions.append(&mut constructor);
    if let Some(drop_kind) = drop_kind {
        instructions.extend(lower_aggregate_drop_instructions_at_location(
            "aggregate field replacement",
            destination,
            destination_offset,
            layout,
            drop_kind,
            context,
        )?);
    }
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(replacement_slot),
        source_offset: 0,
        layout,
    });
    Ok(Some(instructions))
}
