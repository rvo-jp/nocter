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
    let (identifier, outer_failure_mode) = match unwrap_group(&otherwise.value) {
        Expr::Identifier(identifier) => (identifier, None),
        Expr::Propagate(propagation) => {
            let Expr::Identifier(identifier) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            (identifier, Some(propagating_failure_mode(context)?))
        }
        Expr::Force(force) => {
            let Expr::Identifier(identifier) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            (identifier, Some(FallibleFailureMode::Trap))
        }
        Expr::Catch(catch) => {
            let Expr::Identifier(identifier) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            (
                identifier,
                Some(lower_catch_failure_mode(
                    catch,
                    context,
                    outcome_destination_reserved_words(destination),
                )?),
            )
        }
        _ => return Ok(None),
    };
    let Some(local) = context.outcome_local(&identifier.name) else {
        return Ok(None);
    };
    let optional_layer_index = usize::from(outer_failure_mode.is_some());
    if local.storage.layers.len() != optional_layer_index + 1
        || local.storage.layers[optional_layer_index].layer != OutcomeLayer::Optional
        || (outer_failure_mode.is_some() && local.storage.layers[0].layer != OutcomeLayer::Fallible)
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
    let layer = local.storage.layers[optional_layer_index];
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
    let optional_check = Instruction::IfStoredOutcomeTag {
        source: AggregateLocation::Slot(local.slot_index),
        tag_offset,
        success_instructions: vec![Instruction::LoadStoredOutcomePayload {
            destination,
            source: AggregateLocation::Slot(local.slot_index),
            offset: payload_offset,
        }],
        outcome_instructions,
    };
    if let Some(failure_mode) = outer_failure_mode {
        let outer = local.storage.layers[0];
        return Ok(Some(vec![Instruction::CheckStoredFallible {
            source: AggregateLocation::Slot(local.slot_index),
            tag_offset: u32::try_from(outer.tag_offset).map_err(|_| {
                vec![Diagnostic::error(
                    "E8008",
                    "stored outcome tag offset exceeds u32",
                )]
            })?,
            error_offset: u32::try_from(
                outer
                    .failure_offset
                    .expect("fallible layer has error storage"),
            )
            .map_err(|_| {
                vec![Diagnostic::error(
                    "E8008",
                    "stored outcome error offset exceeds u32",
                )]
            })?,
            success_instructions: vec![optional_check],
            failure_mode,
        }]));
    }
    Ok(Some(vec![optional_check]))
}

fn outcome_destination_reserved_words(destination: ComposedOutcomeDestination) -> usize {
    match destination {
        ComposedOutcomeDestination::I32(I32Location::Local(index))
        | ComposedOutcomeDestination::U8(U8Location::Local(index))
        | ComposedOutcomeDestination::Usize(UsizeLocation::Local(index))
        | ComposedOutcomeDestination::Borrow(UsizeLocation::Local(index))
        | ComposedOutcomeDestination::Bool(BoolLocation::Local(index)) => index + 1,
        ComposedOutcomeDestination::Str(StrLocation::Local(index))
        | ComposedOutcomeDestination::Slice(SliceLocation::Local(index)) => index + 2,
        _ => 0,
    }
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
    if let Expr::Member(member) = unwrap_group(&statement.initializer)
        && let Expr::Identifier(root) = unwrap_group(&member.object)
        && let Some(access) = context.aggregate_field(&root.name, &member.member)
        && let AggregateFieldKind::Outcome {
            storage,
            payload_type,
        } = access.kind
    {
        if !access.is_copy {
            return Err(vec![Diagnostic::error(
                "E8008",
                "stored outcome member binding requires `move` for a move-only payload",
            )]);
        }
        let slot_index = context.reserve_aggregate_slot_index();
        let instructions = vec![
            Instruction::ReserveAggregateSlot {
                slot_index,
                layout: storage.layout,
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(slot_index),
                destination_offset: 0,
                source: access.source,
                source_offset: access.offset,
                layout: storage.layout,
            },
        ];
        context.define_outcome_local_at_slot(
            statement.name.clone(),
            slot_index,
            storage,
            payload_type,
            true,
            None,
        );
        return Ok(Some(instructions));
    }
    if let Expr::Index(index) = unwrap_group(&statement.initializer) {
        let lowered = {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let access = fixed_array_element_access(index, context, &mut temporaries, || {
                vec![Diagnostic::error(
                    "E8008",
                    "stored outcome fixed-array index cannot be lowered",
                )]
            })?;
            let Some(access) = access else {
                return Ok(None);
            };
            let AbiType::Outcome { layout } = access.element else {
                return Ok(None);
            };
            if access.out_of_bounds {
                return Err(vec![Diagnostic::error(
                    "E8008",
                    "stored outcome fixed-array index is out of bounds",
                )]);
            }
            let Some(ty) = context.expression_type_expr(statement.initializer.span()) else {
                return Ok(None);
            };
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Ok(None);
            };
            let shape = outcome_shape_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            });
            let Some(storage) = shape.storage_layout(
                abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok()
                .map(|value| value.layout)
                .unwrap_or(layout),
            ) else {
                return Ok(None);
            };
            if storage.layout != layout {
                return Ok(None);
            }
            let Some(payload_type) =
                return_type_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
            else {
                return Ok(None);
            };
            (access, storage, payload_type, ty)
        };
        let (access, storage, payload_type, ty) = lowered;
        let is_copy = context.resolved_calls().is_some_and(|(_, resolved)| {
            type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            })
        });
        if !is_copy {
            return Err(vec![Diagnostic::error(
                "E8008",
                "stored outcome fixed-array binding requires a copyable payload",
            )]);
        }
        let slot_index = context.reserve_aggregate_slot_index();
        let mut instructions = access.instructions;
        instructions.push(Instruction::ReserveAggregateSlot {
            slot_index,
            layout: storage.layout,
        });
        instructions.push(Instruction::CopyAggregateRange {
            destination: AggregateLocation::Slot(slot_index),
            destination_offset: 0,
            source: access.source,
            source_offset: access.offset,
            layout: storage.layout,
        });
        context.define_outcome_local_at_slot(
            statement.name.clone(),
            slot_index,
            storage,
            payload_type,
            true,
            None,
        );
        return Ok(Some(instructions));
    }
    if let Some((source_name, moved)) = outcome_identifier_initializer(&statement.initializer) {
        let Some(source) = context.outcome_local(source_name) else {
            return Ok(None);
        };
        if !source.is_live || (!moved && !source.is_copy) {
            return Err(vec![Diagnostic::error(
                "E8008",
                "stored outcome binding requires a live copy value or an explicit `move`",
            )]);
        }
        let slot_index = context.reserve_aggregate_slot_index();
        context.define_outcome_local_at_slot(
            statement.name.clone(),
            slot_index,
            source.storage.clone(),
            source.payload_type.clone(),
            source.is_copy,
            source.drop_kind.clone(),
        );
        if moved {
            context.mark_outcome_local_moved(source_name);
        }
        return Ok(Some(vec![
            Instruction::ReserveAggregateSlot {
                slot_index,
                layout: source.storage.layout,
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(slot_index),
                source: AggregateLocation::Slot(source.slot_index),
                layout: source.storage.layout,
            },
        ]));
    }

    let Expr::Call(call) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
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
    let drop_kind = context
        .aggregate_drop_for_type_expr(&shape.payload)
        .map(|payload| {
            AggregateDrop::Outcome(OutcomeDrop {
                storage: storage.clone(),
                payload: Box::new(payload),
            })
        });
    context.define_outcome_local_at_slot(
        statement.name.clone(),
        slot_index,
        storage,
        payload_type,
        is_copy,
        drop_kind,
    );
    Ok(Some(instructions))
}

pub(super) fn lower_outcome_assignment(
    target: &crate::ast::IdentifierExpr,
    value: &Expr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(destination) = context.outcome_local(&target.name) else {
        return Ok(None);
    };
    if !destination.is_live {
        return Err(vec![Diagnostic::error(
            "E8008",
            "cannot assign through a moved stored outcome",
        )]);
    }

    if let Some((source_name, moved)) = outcome_identifier_initializer(value) {
        let Some(source) = context.outcome_local(source_name) else {
            return Ok(None);
        };
        if source.storage != destination.storage || !source.is_live || (!moved && !source.is_copy) {
            return Err(vec![Diagnostic::error(
                "E8008",
                "stored outcome assignment requires the same shape and a live copy or move source",
            )]);
        }
        if moved {
            context.mark_outcome_local_moved(source_name);
        }
        let replacement_slot = context.reserve_aggregate_slot_index();
        let mut instructions = vec![
            Instruction::ReserveAggregateSlot {
                slot_index: replacement_slot,
                layout: destination.storage.layout,
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(replacement_slot),
                source: AggregateLocation::Slot(source.slot_index),
                layout: destination.storage.layout,
            },
        ];
        instructions.extend(lower_outcome_replacement_drop(&destination, context)?);
        instructions.push(Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(destination.slot_index),
            source: AggregateLocation::Slot(replacement_slot),
            layout: destination.storage.layout,
        });
        return Ok(Some(instructions));
    }

    let Expr::Call(call) = unwrap_group(value) else {
        return Ok(None);
    };
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
    let payload_abi = abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
        context.resolved_source(source)
    })
    .ok();
    let Some(storage) = payload_abi.and_then(|payload| shape.storage_layout(payload.layout)) else {
        return Ok(None);
    };
    if storage != destination.storage {
        return Err(vec![Diagnostic::error(
            "E8008",
            "stored outcome assignment call has a different storage shape",
        )]);
    }
    let Some((call_target, callee_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) = lower_call_arguments_to_scalar_arguments_with_temporaries(
        call,
        &call_target,
        &callee_name,
        context,
        &mut temporaries,
    )?;
    let replacement_slot = context.reserve_aggregate_slot_index();
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout: destination.storage.layout,
    });
    instructions.push(Instruction::CallStoredOutcome {
        destination: AggregateLocation::Slot(replacement_slot),
        target: call_target,
        arguments,
        storage,
        payload_type: destination.payload_type.clone(),
    });
    instructions.extend(lower_outcome_replacement_drop(&destination, context)?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination.slot_index),
        source: AggregateLocation::Slot(replacement_slot),
        layout: destination.storage.layout,
    });
    Ok(Some(instructions))
}

fn lower_outcome_replacement_drop(
    destination: &OutcomeLocal,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(drop_kind) = &destination.drop_kind else {
        return Ok(Vec::new());
    };
    lower_aggregate_drop_instructions(
        "stored outcome replacement",
        destination.slot_index,
        destination.storage.layout,
        drop_kind,
        context,
    )
}

fn outcome_identifier_initializer(expression: &Expr) -> Option<(&str, bool)> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some((&identifier.name, false)),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return None;
            };
            Some((&identifier.name, true))
        }
        _ => None,
    }
}
