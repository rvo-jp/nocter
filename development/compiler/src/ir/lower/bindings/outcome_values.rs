use super::*;

pub(super) fn lower_stored_optional_otherwise<F>(
    value: &Expr,
    destination: ComposedOutcomeDestination,
    context: &LoweringContext,
    lower_result: F,
    unsupported_message: &'static str,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
{
    let Expr::Otherwise(otherwise) = unwrap_group(value) else {
        return Ok(None);
    };
    let Expr::Identifier(identifier) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    let Some(local) = context.outcome_local(&identifier.name) else {
        return Ok(None);
    };
    if local.storage.layers.len() != 1
        || local.storage.layers[0].layer != OutcomeLayer::Optional
        || !outcome_payload_destination_matches(&local.payload_type, destination)
    {
        return Ok(None);
    }

    let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
        &otherwise.fallback,
        context,
        None,
        lower_result,
        unsupported_message,
    )?;
    let outcome_instructions = match failure_mode {
        FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions } => instructions,
        _ => {
            return Err(vec![Diagnostic::error(
                "E8008",
                "stored optional fallback produced an invalid control mode",
            )]);
        }
    };
    let layer = local.storage.layers[0];
    let tag_offset = u32::try_from(layer.tag_offset).map_err(|_| {
        vec![Diagnostic::error(
            "E8008",
            "stored outcome tag offset exceeds u32",
        )]
    })?;
    let payload_offset = u32::try_from(local.storage.payload_offset).map_err(|_| {
        vec![Diagnostic::error(
            "E8008",
            "stored outcome payload offset exceeds u32",
        )]
    })?;
    Ok(Some(vec![Instruction::IfStoredOutcomeTag {
        source: AggregateLocation::Slot(local.slot_index),
        tag_offset,
        success_instructions: vec![Instruction::LoadStoredOutcomePayload {
            destination,
            source: AggregateLocation::Slot(local.slot_index),
            offset: payload_offset,
        }],
        outcome_instructions,
    }]))
}

fn outcome_payload_destination_matches(
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

pub(super) fn lower_outcome_local_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Call(call) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
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

    let payload_abi = abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|error| {
        vec![Diagnostic::error(
            "E8008",
            format!("cannot lay out stored outcome payload: {error:?}"),
        )]
    })?;
    let storage = shape.storage_layout(payload_abi.layout).ok_or_else(|| {
        vec![Diagnostic::error(
            "E8008",
            "stored outcome has an unsupported layer shape",
        )]
    })?;
    let payload_type =
        return_type_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
            context.resolved_source(source)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E8008",
                "stored outcome payload is not supported by native lowering",
            )]
        })?;
    let Some((target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };

    let slot_index = context.reserve_aggregate_slot_index();
    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) = lower_call_arguments_to_scalar_arguments_with_temporaries(
        call,
        &target,
        &callee_name,
        context,
        &mut temporaries,
    )?;
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index,
        layout: storage.layout,
    });
    instructions.push(Instruction::CallStoredOutcome {
        destination: AggregateLocation::Slot(slot_index),
        target,
        arguments,
        storage: storage.clone(),
        payload_type: payload_type.clone(),
    });

    let is_copy =
        matches!(
            payload_type,
            Type::I32
                | Type::U8
                | Type::Usize
                | Type::Bool
                | Type::Str
                | Type::Slice { .. }
                | Type::Borrow { .. }
        ) || type_expr_is_copy_aggregate_value_with_resolver(&shape.payload, resolved, |source| {
            context.resolved_source(source)
        });
    context.define_outcome_local_at_slot(
        statement.name.clone(),
        slot_index,
        storage,
        payload_type,
        is_copy,
    );
    Ok(Some(instructions))
}
