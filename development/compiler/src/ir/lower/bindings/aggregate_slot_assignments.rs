use super::*;

pub(super) fn lower_aggregate_assignment(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    expression: &Expr,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if aggregate_assignment_moves_from_slot(expression, slot_index, context) {
        return Err(unsupported_assignment_diagnostic());
    }

    let replacement_drop = replacement_drop_for_aggregate_slot(slot_index, context)?;
    if replacement_drop.is_empty() {
        return lower_aggregate_assignment_to_slot(
            slot_index,
            layout,
            target_type,
            expression,
            context,
        );
    }

    if let Expr::ArrayLiteral(literal) = unwrap_group(expression)
        && array_literal_requires_runtime_progress(literal)
        && let Some(target) = context.aggregate_local_by_slot(slot_index)
        && let Some(drop_kind @ AggregateDrop::Array(_)) = target.drop_kind
    {
        return lower_tracked_aggregate_array_replacement(
            slot_index,
            layout,
            target_type,
            literal,
            drop_kind,
            replacement_drop,
            context,
        );
    }

    if let Expr::StructLiteral(literal) = unwrap_group(expression)
        && let Some(target) = context.aggregate_local_by_slot(slot_index)
        && let Some(drop_kind @ (AggregateDrop::Direct(_) | AggregateDrop::Struct(_))) =
            target.drop_kind
    {
        return lower_tracked_aggregate_struct_replacement(
            slot_index,
            layout,
            literal,
            drop_kind,
            replacement_drop,
            context,
        );
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let replacement_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    instructions.extend(lower_aggregate_assignment_to_slot(
        replacement_slot,
        layout,
        target_type,
        expression,
        context,
    )?);
    instructions.extend(replacement_drop);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(slot_index),
        source: AggregateLocation::Slot(replacement_slot),
        layout,
    });
    Ok(instructions)
}

fn lower_tracked_aggregate_struct_replacement(
    destination_slot: usize,
    layout: ValueLayout,
    literal: &crate::ast::StructLiteralExpr,
    drop_kind: AggregateDrop,
    replacement_drop: Vec<Instruction>,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let value = abi_value_from_type_expr(&literal.ty, resolved)
        .map_err(|_error| unsupported_assignment_diagnostic())?;
    let AbiType::Struct(fields) = &value.ty else {
        return Err(unsupported_assignment_diagnostic());
    };
    if value.layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let replacement_slot = context.reserve_aggregate_slot_index();
    let progress = StructInitializationProgress::new(fields, literal, &drop_kind, context)?;
    if !context.register_temporary_struct_fields_drop(
        replacement_slot,
        layout,
        drop_kind,
        progress.drop_states(),
    ) {
        return Err(unsupported_assignment_diagnostic());
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    instructions.extend(progress.initialize());
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        layout,
        AggregateLocation::Slot(replacement_slot),
        0,
        "E8008",
        "assignments",
        resolved,
        context,
        &mut temporaries,
        Some(&progress),
    );
    context.release_temporary_aggregate_drop(replacement_slot);
    instructions.extend(lowered?);
    instructions.extend(replacement_drop);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(replacement_slot),
        layout,
    });
    Ok(instructions)
}

fn lower_tracked_aggregate_array_replacement(
    destination_slot: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    literal: &crate::ast::ArrayLiteralExpr,
    drop_kind: AggregateDrop,
    replacement_drop: Vec<Instruction>,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let replacement_slot = context.reserve_aggregate_slot_index();
    let progress = ArrayInitializationProgress::new(context.reserve_drop_state_usize_local()?);
    if !context.register_temporary_array_prefix_drop(
        replacement_slot,
        layout,
        drop_kind,
        progress.location(),
    ) {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut instructions = vec![
        Instruction::ReserveAggregateSlot {
            slot_index: replacement_slot,
            layout,
        },
        progress.initialize(),
    ];
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered = lower_aggregate_array_literal_assignment_with_progress(
        replacement_slot,
        layout,
        target_type,
        literal,
        context,
        &mut temporaries,
        Some(progress),
    );
    context.release_temporary_aggregate_drop(replacement_slot);
    instructions.extend(lowered?);
    instructions.extend(replacement_drop);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(replacement_slot),
        layout,
    });
    Ok(instructions)
}

pub(super) fn aggregate_assignment_moves_from_slot(
    expression: &Expr,
    destination_slot: usize,
    context: &LoweringContext,
) -> bool {
    let Expr::Unary(unary) = unwrap_group(expression) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return false;
    };
    context
        .aggregate_slot(&identifier.name)
        .is_some_and(|(slot_index, _layout)| slot_index == destination_slot)
}

pub(super) fn lower_aggregate_assignment_to_slot(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_payload_enum_constructor_assignment(
        slot_index,
        layout,
        target_type,
        expression,
        context,
    )? {
        return Ok(instructions);
    }

    match unwrap_group(expression) {
        Expr::ArrayLiteral(literal) => lower_aggregate_array_literal_assignment(
            slot_index,
            layout,
            target_type,
            literal,
            context,
        ),
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_assignment(slot_index, layout, literal, context)
        }
        Expr::Call(call) => lower_aggregate_call_assignment(slot_index, layout, call, context),
        Expr::Identifier(identifier) => {
            lower_aggregate_copy_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Member(_) => lower_aggregate_member_value_assignment(
            AggregateLocation::Slot(slot_index),
            0,
            layout,
            expression,
            context,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_move_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                propagating_failure_mode(context)?,
                context,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                FallibleFailureMode::Trap,
                context,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_assignment(
                slot_index,
                layout,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        Expr::Otherwise(otherwise) => {
            let expected_abi_type = target_type
                .and_then(|ty| aggregate_assignment_expected_abi_type(ty, layout, context));
            lower_aggregate_optional_otherwise_to_location(
                AggregateLocation::Slot(slot_index),
                0,
                layout,
                expected_abi_type.as_ref(),
                otherwise,
                context,
                unsupported_assignment_diagnostic,
            )
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_payload_enum_constructor_assignment(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((member, _arguments)) = payload_enum_constructor_member_and_arguments(expression)
    else {
        return Ok(None);
    };
    let Some(ty) = target_type else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };
    let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_assignment_diagnostic())?;
    let AbiType::Enum(enum_) = &value.ty else {
        return Ok(None);
    };
    if value.layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }
    if !enum_
        .variants
        .iter()
        .any(|variant| variant.name == member.member)
    {
        return Ok(None);
    }

    lower_payload_enum_constructor_to_location(
        expression,
        &value.ty,
        layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "assignments",
        resolved,
        context,
    )
}

pub(super) fn aggregate_assignment_expected_abi_type(
    target_type: &TypeExpr,
    expected_layout: ValueLayout,
    context: &LoweringContext,
) -> Option<AbiType> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(target_type, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    (value.layout == expected_layout).then_some(value.ty)
}

pub(super) fn lower_aggregate_copy_assignment(
    destination_slot: usize,
    destination_layout: ValueLayout,
    source_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(source) = context.aggregate_local(source_name) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(destination) = context.aggregate_local_by_slot(destination_slot) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if source.layout != destination_layout
        || destination.layout != destination_layout
        || !source.is_copy
        || !destination.is_copy
    {
        return Err(unsupported_assignment_diagnostic());
    }

    Ok(vec![Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(source.slot_index),
        layout: destination_layout,
    }])
}

pub(super) fn lower_aggregate_move_assignment(
    destination_slot: usize,
    destination_layout: ValueLayout,
    source_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(source) = context.aggregate_local(source_name) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let destination = context.aggregate_local_by_slot(destination_slot);
    if source.slot_index == destination_slot
        || source.layout != destination_layout
        || destination.is_some_and(|destination| destination.layout != destination_layout)
        || !supported_aggregate_copy_layout(destination_layout)
    {
        return Err(unsupported_assignment_diagnostic());
    }

    Ok(vec![Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(destination_slot),
        source: AggregateLocation::Slot(source.slot_index),
        layout: destination_layout,
    }])
}

pub(super) fn lower_aggregate_struct_literal_assignment(
    slot_index: usize,
    layout: ValueLayout,
    literal: &crate::ast::StructLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };

    lower_aggregate_struct_literal_to_location(
        literal,
        layout,
        AggregateLocation::Slot(slot_index),
        "E8008",
        "assignments",
        resolved,
        context,
    )
}

pub(super) fn lower_aggregate_array_literal_assignment(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    literal: &crate::ast::ArrayLiteralExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_array_literal_assignment_with_progress(
        slot_index,
        layout,
        target_type,
        literal,
        context,
        &mut temporaries,
        None,
    )
}

fn lower_aggregate_array_literal_assignment_with_progress(
    slot_index: usize,
    layout: ValueLayout,
    target_type: Option<&TypeExpr>,
    literal: &crate::ast::ArrayLiteralExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<ArrayInitializationProgress>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(ty) = target_type else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_assignment_diagnostic())?;
    if !matches!(value.ty, AbiType::Array { .. }) || value.layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    lower_aggregate_array_literal_to_location_with_progress(
        literal,
        &value.ty,
        layout,
        AggregateLocation::Slot(slot_index),
        0,
        "E8008",
        "assignments",
        resolved,
        context,
        temporaries,
        progress,
    )
}

pub(super) fn lower_aggregate_call_assignment(
    slot_index: usize,
    layout: ValueLayout,
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context) {
        let mut temporaries = TemporaryAllocator::new(context)?;
        if let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            &mut temporaries,
        )? {
            return Ok(instructions);
        }
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };

    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok(instructions)
}

pub(super) fn lower_aggregate_fallible_call_assignment(
    slot_index: usize,
    layout: ValueLayout,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };

    let Some(Type::Fallible(success)) = context.call_return_type(&target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(success.as_ref()) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(instructions)
}
