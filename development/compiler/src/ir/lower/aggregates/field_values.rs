use super::literals::lower_aggregate_array_literal_to_location_at_offset_with_temporaries;
use super::*;

pub(super) fn lower_aggregate_field_to_location(
    field_type: &AbiType,
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match field_type {
        AbiType::I64 | AbiType::Isize => {
            let value = lower_i64_literal(expression)? as u64;
            Ok(vec![Instruction::StoreAggregateUsize {
                destination,
                offset,
                value: UsizeValue::Const(value),
            }])
        }
        AbiType::U64 => Ok(vec![Instruction::StoreAggregateUsize {
            destination,
            offset,
            value: UsizeValue::Const(lower_u64_literal(expression)?),
        }]),
        AbiType::Usize => {
            let (mut instructions, value) = lower_usize_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::I32 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let (mut instructions, value) = lower_i32_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateI32 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::I8 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU8 {
                destination,
                offset,
                value: U8Value::Const(lower_i8_literal(expression)? as u8),
            }])
        }
        AbiType::I16 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU16 {
                destination,
                offset,
                value: lower_i16_literal(expression)? as u16,
            }])
        }
        AbiType::U16 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU16 {
                destination,
                offset,
                value: lower_u16_literal(expression)?,
            }])
        }
        AbiType::U32 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            Ok(vec![Instruction::StoreAggregateU32 {
                destination,
                offset,
                value: lower_u32_literal(expression)?,
            }])
        }
        AbiType::U8 => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let (mut instructions, value) = lower_u8_expression_to_word(expression, context)?;
            instructions.push(Instruction::StoreAggregateU8 {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::Bool => {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
            let mut lowered = lower_bool_expression_to_value(expression, context, diagnostic_code)?;
            lowered.instructions.push(Instruction::StoreAggregateBool {
                destination,
                offset,
                value: lowered.value,
            });
            Ok(lowered.instructions)
        }
        AbiType::StrView => lower_str_view_field_to_location(
            expression,
            destination,
            offset,
            diagnostic_code,
            subject,
            context,
            temporaries,
        ),
        AbiType::SliceView => lower_slice_view_field_to_location(
            expression,
            destination,
            offset,
            diagnostic_code,
            subject,
            context,
            temporaries,
        ),
        AbiType::Pointer => {
            let (mut instructions, value) = lower_aggregate_pointer_field_value(
                expression,
                diagnostic_code,
                subject,
                context,
                temporaries,
            )?;
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value,
            });
            Ok(instructions)
        }
        AbiType::Array { .. } => {
            let expected_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;

            match expression {
                Expr::ArrayLiteral(literal) => {
                    lower_aggregate_array_literal_to_location_at_offset_with_temporaries(
                        literal,
                        field_type,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        resolved,
                        context,
                        temporaries,
                    )
                }
                Expr::Identifier(identifier) => {
                    let Some(source) = context.aggregate_local(&identifier.name) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    if source.layout != expected_layout || !source.is_copy {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    Ok(vec![Instruction::CopyAggregateRange {
                        destination,
                        destination_offset: offset,
                        source: AggregateLocation::Slot(source.slot_index),
                        source_offset: 0,
                        layout: expected_layout,
                    }])
                }
                Expr::Call(call) => lower_aggregate_call_field_value_to_location(
                    call,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Propagate(propagation) => {
                    let Some(call) = call_expression(&propagation.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        propagating_failure_mode(context)?,
                    )
                }
                Expr::Force(force) => {
                    let Some(call) = call_expression(&force.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        FallibleFailureMode::Trap,
                    )
                }
                Expr::Catch(catch) => {
                    let Some(call) = call_expression(&catch.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        lower_catch_failure_mode(catch, context, 0)?,
                    )
                }
                Expr::Otherwise(otherwise) => lower_aggregate_optional_otherwise_to_location(
                    destination,
                    offset,
                    expected_layout,
                    Some(field_type),
                    otherwise,
                    context,
                    || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
                ),
                Expr::Member(_) => lower_aggregate_member_field_value_to_location(
                    expression,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Group(group) => lower_aggregate_field_to_location(
                    field_type,
                    &group.expression,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                ),
                _ => Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                )),
            }
        }
        AbiType::Struct(fields) => {
            let expected_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;

            match expression {
                Expr::StructLiteral(literal) => {
                    let actual =
                        abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
                            unsupported_aggregate_struct_literal_diagnostic(
                                diagnostic_code,
                                subject,
                            )
                        })?;
                    if actual.layout != expected_layout {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    lower_aggregate_struct_fields_to_location(
                        fields,
                        literal,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        resolved,
                        context,
                        temporaries,
                    )
                }
                Expr::Identifier(identifier) => {
                    let Some(source) = context.aggregate_local(&identifier.name) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    if source.layout != expected_layout
                        || !source.is_copy
                        || !supported_aggregate_copy_layout(expected_layout)
                    {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    Ok(vec![Instruction::CopyAggregateRange {
                        destination,
                        destination_offset: offset,
                        source: AggregateLocation::Slot(source.slot_index),
                        source_offset: 0,
                        layout: expected_layout,
                    }])
                }
                Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
                    let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    let Some(source) = context.aggregate_local(&identifier.name) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    if source.layout != expected_layout
                        || !supported_aggregate_copy_layout(expected_layout)
                    {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    }
                    Ok(vec![Instruction::CopyAggregateRange {
                        destination,
                        destination_offset: offset,
                        source: AggregateLocation::Slot(source.slot_index),
                        source_offset: 0,
                        layout: expected_layout,
                    }])
                }
                Expr::Call(call) => lower_aggregate_call_field_value_to_location(
                    call,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Propagate(propagation) => {
                    let Some(call) = call_expression(&propagation.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        propagating_failure_mode(context)?,
                    )
                }
                Expr::Force(force) => {
                    let Some(call) = call_expression(&force.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        FallibleFailureMode::Trap,
                    )
                }
                Expr::Catch(catch) => {
                    let Some(call) = call_expression(&catch.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        lower_catch_failure_mode(catch, context, 0)?,
                    )
                }
                Expr::Otherwise(otherwise) => lower_aggregate_optional_otherwise_to_location(
                    destination,
                    offset,
                    expected_layout,
                    Some(field_type),
                    otherwise,
                    context,
                    || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
                ),
                Expr::Member(_) => lower_aggregate_member_field_value_to_location(
                    expression,
                    expected_layout,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    context,
                    temporaries,
                ),
                Expr::Group(group) => lower_aggregate_field_to_location(
                    field_type,
                    &group.expression,
                    destination,
                    offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                ),
                _ => Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                )),
            }
        }
        _ => Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}

fn lower_str_view_field_to_location(
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut lowered = lower_str_expression_to_value(expression, context, temporaries)?;
    push_store_str_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        temporaries,
        || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
    )?;
    Ok(lowered.instructions)
}

fn lower_slice_view_field_to_location(
    expression: &Expr,
    destination: AggregateLocation,
    offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut lowered = lower_slice_expression_to_value(expression, context, temporaries)?;
    push_store_slice_view_to_aggregate_field(
        &mut lowered.instructions,
        destination,
        offset,
        lowered.value,
        temporaries,
        || unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject),
    )?;
    Ok(lowered.instructions)
}

pub(super) fn lower_aggregate_struct_fields_to_location(
    fields: &[crate::abi::AbiField],
    literal: &StructLiteralExpr,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let struct_layout = layout_struct(fields).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    let field_layouts = fields
        .iter()
        .zip(struct_layout.fields.iter())
        .map(|(field, layout)| (field.name.as_str(), (&field.ty, layout)))
        .collect::<HashMap<_, _>>();

    let mut instructions = Vec::new();
    for field in &literal.fields {
        let Some((field_type, field_layout)) = field_layouts.get(field.name.as_str()) else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        let nested_offset = u32::try_from(field_layout.offset)
            .ok()
            .and_then(|offset| base_offset.checked_add(offset))
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        instructions.extend(lower_aggregate_field_to_location(
            field_type,
            &field.value,
            destination,
            nested_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        )?);
    }
    Ok(instructions)
}

fn lower_aggregate_call_field_value_to_location(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
        && layout == expected_layout
    {
        if destination_offset != 0 {
            let source_slot = temporaries.next_aggregate_slot();
            let Some(mut instructions) = lower_macos_syscall_primitive_call_to_location(
                call,
                AggregateLocation::Slot(source_slot),
                expected_layout,
                context,
                temporaries,
            )?
            else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            let mut staged = vec![Instruction::ReserveAggregateSlot {
                slot_index: source_slot,
                layout,
            }];
            staged.append(&mut instructions);
            staged.push(Instruction::CopyAggregateRange {
                destination,
                destination_offset,
                source: AggregateLocation::Slot(source_slot),
                source_offset: 0,
                layout,
            });
            return Ok(staged);
        }
        let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            destination,
            expected_layout,
            context,
            temporaries,
        )?
        else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        return Ok(instructions);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

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
            temporaries,
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

fn lower_aggregate_fallible_call_field_value_to_location(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

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
            temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
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

fn lower_aggregate_member_field_value_to_location(
    expression: &Expr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(access) = lower_aggregate_member_field_access(expression, context, temporaries)?
    else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = access.kind.copy_aggregate_layout() else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if layout != expected_layout || !access.is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source: access.source,
        source_offset: access.offset,
        layout,
    });
    Ok(instructions)
}

fn call_expression(expression: &Expr) -> Option<&CallExpr> {
    match expression {
        Expr::Call(call) => Some(call),
        Expr::Group(group) => call_expression(&group.expression),
        _ => None,
    }
}

fn macos_syscall_primitive_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}

pub(super) fn validate_direct_aggregate_field_store(
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<(), Vec<Diagnostic>> {
    if matches!(destination, AggregateLocation::DirectReturn) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    Ok(())
}

fn lower_aggregate_pointer_field_value(
    expression: &Expr,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match expression {
        Expr::Call(call)
            if context.primitive_name_for_call(call) == Some("from_addr")
                && call.arguments.len() == 1 =>
        {
            lower_usize_expression_to_word(&call.arguments[0], context)
        }
        Expr::Member(_) => {
            let access = lower_aggregate_member_field_access(expression, context, temporaries)?
                .filter(|access| access.kind == AggregateFieldKind::Usize)
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                })?;
            let destination = temporaries.next_usize()?;
            let mut instructions = access.instructions;
            instructions.push(Instruction::LoadAggregateUsize {
                destination,
                source: access.source,
                offset: access.offset,
            });
            Ok((instructions, UsizeValue::Location(destination)))
        }
        Expr::Group(group) => lower_aggregate_pointer_field_value(
            &group.expression,
            diagnostic_code,
            subject,
            context,
            temporaries,
        ),
        _ => Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}
