use super::*;

pub(super) fn lower_aggregate_field_assignment(
    target: &MemberExpr,
    value: &Expr,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((identifier_name, field_path)) = aggregate_assignment_target_path(target) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(field) = context.aggregate_field(identifier_name, &field_path) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if !field.is_readwrite {
        return Err(unsupported_assignment_diagnostic());
    }
    let destination = field.source;
    let offset = field.offset;
    let field_is_copy = field.is_copy;
    let field_drop_kind = field.drop_kind.clone();
    match field.kind {
        AggregateFieldKind::I8 => Ok(vec![Instruction::StoreAggregateU8 {
            destination,
            offset,
            value: U8Value::Const(lower_i8_literal(value)? as u8),
        }]),
        AggregateFieldKind::I16 => Ok(vec![Instruction::StoreAggregateU16 {
            destination,
            offset,
            value: lower_i16_literal(value)? as u16,
        }]),
        AggregateFieldKind::I32 => {
            if let Some(instructions) =
                lower_i32_otherwise_aggregate_field_assignment(value, destination, offset, context)?
            {
                return Ok(instructions);
            }
            let (mut instructions, value) = lower_i32_expression_to_word(value, context)?;
            instructions.push(Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::I64 | AggregateFieldKind::Isize => {
            Ok(vec![Instruction::StoreAggregateUsize {
                destination,
                offset,
                value: UsizeValue::Const(lower_i64_literal(value)? as u64),
            }])
        }
        AggregateFieldKind::U16 => Ok(vec![Instruction::StoreAggregateU16 {
            destination,
            offset,
            value: lower_u16_literal(value)?,
        }]),
        AggregateFieldKind::U32 => Ok(vec![Instruction::StoreAggregateU32 {
            destination,
            offset,
            value: lower_u32_literal(value)?,
        }]),
        AggregateFieldKind::U64 => Ok(vec![Instruction::StoreAggregateUsize {
            destination,
            offset,
            value: UsizeValue::Const(lower_u64_literal(value)?),
        }]),
        AggregateFieldKind::U8 => {
            if let Some(instructions) =
                lower_u8_otherwise_aggregate_field_assignment(value, destination, offset, context)?
            {
                return Ok(instructions);
            }
            let (mut instructions, value) = lower_u8_expression_to_word(value, context)?;
            instructions.push(Instruction::StoreAggregateU8 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::Usize => {
            if let Some(instructions) = lower_usize_otherwise_aggregate_field_assignment(
                value,
                destination,
                offset,
                context,
            )? {
                return Ok(instructions);
            }
            let (mut instructions, value) = match lower_usize_expression_to_word(value, context) {
                Ok(lowered) => lowered,
                Err(_) if expression_is_pointer_address_value(value, context) => {
                    let mut temporaries = TemporaryAllocator::new(context)?;
                    lower_pointer_address_expression_to_word(value, context, &mut temporaries)?
                }
                Err(diagnostics) => return Err(diagnostics),
            };
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AggregateFieldKind::Bool => {
            if let Some(instructions) = lower_bool_otherwise_aggregate_field_assignment(
                value,
                destination,
                offset,
                context,
            )? {
                return Ok(instructions);
            }
            let mut lowered = lower_bool_expression_to_value(value, context, "E8008")?;
            lowered.instructions.push(Instruction::StoreAggregateBool {
                destination,
                offset,
                value: lowered.value,
            });
            Ok(lowered.instructions)
        }
        AggregateFieldKind::Str => {
            if let Some(instructions) =
                lower_str_otherwise_aggregate_field_assignment(value, destination, offset, context)?
            {
                return Ok(instructions);
            }
            lower_str_aggregate_field_assignment(value, destination, offset, context)
        }
        AggregateFieldKind::Slice(_) => {
            if let Some(instructions) = lower_slice_otherwise_aggregate_field_assignment(
                value,
                destination,
                offset,
                context,
            )? {
                return Ok(instructions);
            }
            lower_slice_aggregate_field_assignment(value, destination, offset, context)
        }
        AggregateFieldKind::Array {
            layout,
            element,
            length,
            ..
        } => lower_aggregate_array_field_assignment(
            destination,
            offset,
            layout,
            element,
            length,
            field_is_copy,
            field_drop_kind,
            value,
            context,
        ),
        AggregateFieldKind::Aggregate { layout, .. } => {
            if field_is_copy {
                lower_aggregate_member_value_assignment(destination, offset, layout, value, context)
            } else {
                if let Some(instructions) = lower_payload_enum_field_replacement(
                    target,
                    destination,
                    offset,
                    layout,
                    field_drop_kind.as_ref(),
                    value,
                    context,
                )? {
                    return Ok(instructions);
                }
                lower_aggregate_member_replacement_assignment(
                    destination,
                    offset,
                    layout,
                    field_drop_kind,
                    value,
                    context,
                )
            }
        }
        AggregateFieldKind::Outcome { storage, .. } => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let replacement_slot = temporaries.next_aggregate_slot();
            let Some(mut instructions) = super::super::aggregates::lower_outcome_field_to_location(
                storage.layout,
                value,
                AggregateLocation::Slot(replacement_slot),
                0,
                context,
                &mut temporaries,
            )?
            else {
                return Err(unsupported_assignment_diagnostic());
            };
            instructions.insert(
                0,
                Instruction::ReserveAggregateSlot {
                    slot_index: replacement_slot,
                    layout: storage.layout,
                },
            );
            if let Some(drop_kind) = &field_drop_kind {
                instructions.extend(lower_aggregate_drop_instructions_at_location(
                    "outcome field replacement",
                    destination,
                    offset,
                    storage.layout,
                    drop_kind,
                    context,
                )?);
            }
            instructions.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset: offset,
                source: AggregateLocation::Slot(replacement_slot),
                source_offset: 0,
                layout: storage.layout,
            });
            Ok(instructions)
        }
    }
}

pub(super) fn lower_aggregate_array_field_assignment(
    destination: AggregateLocation,
    offset: u32,
    layout: ValueLayout,
    element: AbiType,
    length: u64,
    is_copy: bool,
    drop_kind: Option<AggregateDrop>,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !is_copy {
        let Some(drop_kind) = drop_kind else {
            return Err(unsupported_assignment_diagnostic());
        };
        let mut temporaries = TemporaryAllocator::new(context)?;
        let replacement_slot = temporaries.next_aggregate_slot();
        let mut instructions = vec![Instruction::ReserveAggregateSlot {
            slot_index: replacement_slot,
            layout,
        }];
        instructions.extend(lower_aggregate_array_field_value_assignment(
            AggregateLocation::Slot(replacement_slot),
            0,
            layout,
            element,
            length,
            value,
            context,
        )?);
        instructions.extend(lower_aggregate_drop_instructions_at_location(
            "aggregate field replacement",
            destination,
            offset,
            layout,
            &drop_kind,
            context,
        )?);
        instructions.push(Instruction::CopyAggregateRange {
            destination,
            destination_offset: offset,
            source: AggregateLocation::Slot(replacement_slot),
            source_offset: 0,
            layout,
        });
        return Ok(instructions);
    }

    lower_aggregate_array_field_value_assignment(
        destination,
        offset,
        layout,
        element,
        length,
        value,
        context,
    )
}

fn lower_aggregate_array_field_value_assignment(
    destination: AggregateLocation,
    offset: u32,
    layout: ValueLayout,
    element: AbiType,
    length: u64,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expected_type = AbiType::Array {
        element: Box::new(element),
        length,
    };
    if let Expr::ArrayLiteral(literal) = unwrap_group(value) {
        let Some((_root_source, resolved)) = context.resolved_calls() else {
            return Err(unsupported_assignment_diagnostic());
        };
        return lower_aggregate_array_literal_to_location_at_offset(
            literal,
            &expected_type,
            layout,
            destination,
            offset,
            "E8008",
            "assignments",
            resolved,
            context,
        );
    }
    if let Expr::Otherwise(otherwise) = unwrap_group(value) {
        return lower_aggregate_optional_otherwise_to_location(
            destination,
            offset,
            layout,
            Some(&expected_type),
            otherwise,
            context,
            unsupported_assignment_diagnostic,
        );
    }

    lower_aggregate_member_value_assignment(destination, offset, layout, value, context)
}

pub(super) fn lower_str_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
    push_store_str_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(lowered.instructions)
}

pub(super) fn lower_slice_aggregate_field_assignment(
    value: &Expr,
    destination: AggregateLocation,
    offset: u32,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut lowered = lower_slice_expression_to_value(value, context, &mut temporaries)?;
    push_store_slice_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?;
    Ok(lowered.instructions)
}

pub(super) fn lower_aggregate_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_assignment_diagnostic());
    }

    match unwrap_group(value) {
        Expr::StructLiteral(literal) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_struct_literal_to_location_at_offset(
                literal,
                layout,
                destination,
                destination_offset,
                "E8008",
                "assignments",
                resolved,
                context,
            )
        }
        Expr::Identifier(identifier) => {
            let Some(source) = context.aggregate_local(&identifier.name) else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source.layout != layout || !source.is_copy {
                return Err(unsupported_assignment_diagnostic());
            }
            Ok(vec![Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source.slot_index),
                source_offset: 0,
                layout,
            }])
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            let Some(source) = context.aggregate_local(&identifier.name) else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source.layout != layout {
                return Err(unsupported_assignment_diagnostic());
            }
            Ok(vec![Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source.slot_index),
                source_offset: 0,
                layout,
            }])
        }
        Expr::Call(call) => lower_aggregate_call_member_value_assignment(
            destination,
            destination_offset,
            layout,
            call,
            context,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
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
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
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
            lower_aggregate_fallible_call_member_value_assignment(
                destination,
                destination_offset,
                layout,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        Expr::Otherwise(otherwise) => lower_aggregate_optional_otherwise_to_location(
            destination,
            destination_offset,
            layout,
            None,
            otherwise,
            context,
            unsupported_assignment_diagnostic,
        ),
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let access = lower_aggregate_member_field_access(value, context, &mut temporaries)?
                .ok_or_else(unsupported_assignment_diagnostic)?;
            let source_location = access.source;
            let source_offset = access.offset;
            let source_is_copy = access.is_copy;
            let Some(source_layout) = access.kind.copy_aggregate_layout() else {
                return Err(unsupported_assignment_diagnostic());
            };
            if source_layout != layout || !source_is_copy {
                return Err(unsupported_assignment_diagnostic());
            }
            let mut instructions = access.instructions;
            instructions.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: source_location,
                source_offset,
                layout,
            });
            Ok(instructions)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_aggregate_member_replacement_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    drop_kind: Option<AggregateDrop>,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let replacement_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout,
    }];
    instructions.extend(lower_aggregate_member_value_assignment(
        AggregateLocation::Slot(replacement_slot),
        0,
        layout,
        value,
        context,
    )?);
    if let Some(drop_kind) = drop_kind {
        instructions.extend(lower_aggregate_drop_instructions_at_location(
            "aggregate field replacement",
            destination,
            destination_offset,
            layout,
            &drop_kind,
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
    Ok(instructions)
}

pub(super) fn lower_aggregate_call_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: source_slot,
        layout,
    }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        layout,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(source_slot),
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_fallible_call_member_value_assignment(
    destination: AggregateLocation,
    destination_offset: u32,
    layout: ValueLayout,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_assignment_diagnostic());
    };
    let Some(callee_layout) = aggregate_type_layout(success.as_ref()) else {
        return Err(unsupported_assignment_diagnostic());
    };
    if callee_layout != layout {
        return Err(unsupported_assignment_diagnostic());
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index: source_slot,
        layout,
    }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success.as_ref(),
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: AggregateLocation::Slot(source_slot),
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

pub(super) fn aggregate_assignment_target_path(target: &MemberExpr) -> Option<(&str, String)> {
    let (identifier_name, mut fields) = aggregate_assignment_root_and_path(&target.object)?;
    fields.push(target.member.as_str());
    Some((identifier_name, fields.join(".")))
}

pub(super) fn aggregate_assignment_root_and_path(expression: &Expr) -> Option<(&str, Vec<&str>)> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some((&identifier.name, Vec::new())),
        Expr::Member(member) => {
            let (identifier_name, mut fields) = aggregate_assignment_root_and_path(&member.object)?;
            fields.push(member.member.as_str());
            Some((identifier_name, fields))
        }
        _ => None,
    }
}
