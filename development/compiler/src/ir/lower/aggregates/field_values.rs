use super::literals::{
    lower_aggregate_array_literal_to_location_at_offset_with_temporaries,
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries,
    lower_payload_enum_constructor_to_location_at_offset_with_progress,
};
use super::*;
use crate::ir::UsizeLocation;

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
    if let Expr::InterpolatedString(interpolated) = unwrap_field_value_group(expression) {
        let expected_layout = layout_of(field_type).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let result_ty = context
            .expression_type_expr(interpolated.span)
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        let drop_kind = context
            .aggregate_drop_for_type_expr(&result_ty)
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        let source_slot = temporaries.next_aggregate_slot();
        let mut interpolation_context = context.clone();
        if !interpolation_context.register_or_complete_temporary_aggregate_drop(
            source_slot,
            expected_layout,
            drop_kind,
        ) {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        }
        let mut instructions = vec![Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: expected_layout,
        }];
        instructions.extend(
            crate::ir::lower::interpolation::lower_interpolated_string_to_slot(
                interpolated,
                source_slot,
                &interpolation_context,
            )?,
        );
        instructions.push(Instruction::CopyAggregateRange {
            destination,
            destination_offset: offset,
            source: AggregateLocation::Slot(source_slot),
            source_offset: 0,
            layout: expected_layout,
        });
        return Ok(instructions);
    }
    if matches!(
        unwrap_field_value_group(expression),
        Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_)
    ) {
        let expected_layout = layout_of(field_type).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let source_slot = temporaries.next_aggregate_slot();
        let mut instructions = vec![Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: expected_layout,
        }];
        instructions.extend(
            crate::ir::lower::typed_literals::lower_typed_literal_to_location(
                expression,
                AggregateLocation::Slot(source_slot),
                context,
            )?
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?,
        );
        instructions.push(Instruction::CopyAggregateRange {
            destination,
            destination_offset: offset,
            source: AggregateLocation::Slot(source_slot),
            source_offset: 0,
            layout: expected_layout,
        });
        return Ok(instructions);
    }
    if let Some(kind) = field_type
        .integer_type()
        .filter(|kind| !kind.legacy_ir_type())
    {
        if kind.bit_width() < 64 {
            validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;
        }
        let mut lowered =
            lower_integer_expression_to_value(expression, kind, context, temporaries)?;
        lowered
            .instructions
            .push(Instruction::StoreAggregateInteger {
                kind,
                destination,
                offset,
                value: lowered.value,
            });
        return Ok(lowered.instructions);
    }
    match field_type {
        AbiType::I8
        | AbiType::I16
        | AbiType::I64
        | AbiType::Isize
        | AbiType::U16
        | AbiType::U32
        | AbiType::U64 => unreachable!("non-legacy integer fields are lowered uniformly"),
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
        AbiType::Borrow => {
            if context.coercion_plan(expression.span()).is_some() {
                let pointer = temporaries.next_usize()?;
                let mut instructions = lower_borrow_coercion_to_location_with_temporaries(
                    expression,
                    pointer,
                    context,
                    temporaries,
                )
                .expect("checked coercion plan must lower")?;
                instructions.push(Instruction::StoreAggregateUsize {
                    destination,
                    offset,
                    value: UsizeValue::Location(pointer),
                });
                return Ok(instructions);
            }
            let moved_value = match unwrap_field_value_group(expression) {
                Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
                    unwrap_field_value_group(&unary.operand)
                }
                expression => expression,
            };
            if let Expr::Identifier(identifier) = moved_value {
                let pointer = context
                    .borrow_parameter(&identifier.name)
                    .map(|parameter| {
                        UsizeValue::Location(UsizeLocation::Parameter(parameter.parameter_index))
                    })
                    .or_else(|| {
                        context
                            .borrow_local(&identifier.name)
                            .map(|(pointer, _, _)| UsizeValue::Location(pointer))
                    });
                if let Some(pointer) = pointer {
                    return Ok(vec![Instruction::StoreAggregateUsize {
                        destination,
                        offset,
                        value: pointer,
                    }]);
                }
            }
            let Expr::Borrow(borrow) = unwrap_field_value_group(expression) else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            let Expr::Identifier(identifier) = unwrap_field_value_group(&borrow.expression) else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            let inner_ty = context
                .local_binding_type_expr_for_identifier(identifier)
                .and_then(|ty| context.ir_type_for_type_expr(&ty))
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                })?;
            let borrow_ty = Type::Borrow {
                is_readwrite: borrow.is_readwrite,
                inner: Box::new(inner_ty),
            };
            let pointer = temporaries.next_usize()?;
            let mut instructions =
                lower_borrow_expression_to_location(expression, pointer, &borrow_ty, context)?;
            instructions.push(Instruction::StoreAggregateUsize {
                destination,
                offset,
                value: UsizeValue::Location(pointer),
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
                Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
                    let Expr::Identifier(identifier) = unwrap_field_value_group(&unary.operand)
                    else {
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
                    if source.layout != expected_layout || source.is_copy {
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
                        propagating_outcome_mode(&propagation.expression, context)?,
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
                        OutcomeFailureMode::Trap,
                    )
                }
                Expr::Catch(catch) => {
                    let Some(call) = call_expression(&catch.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location_with(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        |source, success_type, context| {
                            let Some((_root_source, resolved)) = context.resolved_calls() else {
                                return Err(unsupported_aggregate_struct_literal_diagnostic(
                                    diagnostic_code,
                                    subject,
                                ));
                            };
                            lower_value_catch_failure_mode(
                                catch,
                                context,
                                0,
                                None,
                                |result, context| {
                                    lower_aggregate_return_expression_to_location(
                                        result,
                                        success_type,
                                        source,
                                        context.function_name(),
                                        resolved,
                                        context,
                                    )
                                },
                                "aggregate field `catch` fallback must produce the field type or exit",
                            )
                        },
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
            if expected_layout.size == 0 {
                return Ok(Vec::new());
            }

            match expression {
                Expr::StructLiteral(literal) => {
                    let actual = context
                        .abi_value_for_type_expr(&literal.ty)
                        .ok_or_else(|| {
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
                        None,
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
                        propagating_outcome_mode(&propagation.expression, context)?,
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
                        OutcomeFailureMode::Trap,
                    )
                }
                Expr::Catch(catch) => {
                    let Some(call) = call_expression(&catch.expression) else {
                        return Err(unsupported_aggregate_struct_literal_diagnostic(
                            diagnostic_code,
                            subject,
                        ));
                    };
                    lower_aggregate_fallible_call_field_value_to_location_with(
                        call,
                        expected_layout,
                        destination,
                        offset,
                        diagnostic_code,
                        subject,
                        context,
                        temporaries,
                        |source, success_type, context| {
                            let Some((_root_source, resolved)) = context.resolved_calls() else {
                                return Err(unsupported_aggregate_struct_literal_diagnostic(
                                    diagnostic_code,
                                    subject,
                                ));
                            };
                            lower_value_catch_failure_mode(
                                catch,
                                context,
                                0,
                                None,
                                |result, context| {
                                    lower_aggregate_return_expression_to_location(
                                        result,
                                        success_type,
                                        source,
                                        context.function_name(),
                                        resolved,
                                        context,
                                    )
                                },
                                "aggregate field `catch` fallback must produce the field type or exit",
                            )
                        },
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
        AbiType::Enum(_) => lower_enum_field_value_to_location(
            field_type,
            expression,
            destination,
            offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        ),
        AbiType::Outcome { layout } => {
            let expression_type =
                context
                    .expression_type_expr(expression.span())
                    .ok_or_else(|| {
                        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                    })?;
            let shape = outcome_shape_with_resolver(&expression_type, resolved, |source| {
                context.resolved_source(source)
            });
            let storage = context
                .abi_value_for_type_expr(&shape.payload)
                .and_then(|payload| shape.storage_layout(payload.layout))
                .filter(|storage| storage.layout == *layout)
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                })?;
            lower_outcome_field_to_location(
                &storage,
                expression,
                destination,
                offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
            )?
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })
        }
    }
}

fn unwrap_field_value_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_field_value_group(&group.expression),
        _ => expression,
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
    progress: Option<&StructInitializationProgress>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let struct_layout = layout_struct(fields).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    let field_layouts = fields
        .iter()
        .zip(struct_layout.fields.iter())
        .map(|(field, layout)| (field.name.as_str(), (&field.ty, layout)))
        .collect::<HashMap<_, _>>();
    let literal_type = context.specialize_type_expr(&literal.ty);
    let outcome_fields = aggregate_fields_from_type_expr_with_resolver(
        &literal_type,
        literal.span.source,
        resolved,
        |source| context.resolved_source(source),
    )
    .unwrap_or_default()
    .into_iter()
    .filter_map(|field| match field.kind {
        AggregateFieldKind::Outcome { storage, .. } => Some((field.name, storage)),
        _ => None,
    })
    .collect::<HashMap<_, _>>();

    let mut instructions = Vec::new();
    for field in &literal.fields {
        let Some((field_type, field_layout)) = field_layouts.get(field.name.as_str()) else {
            return Err(unsupported_aggregate_struct_literal_diagnostic(
                diagnostic_code,
                subject,
            ));
        };
        let field_offset = u32::try_from(field_layout.offset).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let nested_offset = base_offset.checked_add(field_offset).ok_or_else(|| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let array_progress =
            progress.and_then(|progress| progress.array_field_progress(field_offset));
        let struct_progress =
            progress.and_then(|progress| progress.struct_field_progress(field_offset));
        let payload_progress =
            progress.and_then(|progress| progress.payload_field_progress(field_offset));
        if let (Some(struct_progress), AbiType::Struct(_), Expr::StructLiteral(literal)) = (
            struct_progress,
            field_type,
            unwrap_field_value_group(&field.value),
        ) {
            let field_value_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            instructions.extend(
                lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
                    literal,
                    field_value_layout,
                    destination,
                    nested_offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                    Some(&struct_progress),
                )?,
            );
        } else if let (Some(array_progress), AbiType::Array { .. }, Expr::ArrayLiteral(literal)) = (
            array_progress,
            field_type,
            unwrap_field_value_group(&field.value),
        ) {
            let field_value_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            instructions.extend(lower_aggregate_array_literal_to_location_with_progress(
                literal,
                field_type,
                field_value_layout,
                destination,
                nested_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
                Some(&array_progress),
            )?);
        } else if let (Some(payload_progress), AbiType::Enum(_)) = (payload_progress, field_type) {
            let field_value_layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            let Some(mut payload_instructions) =
                lower_payload_enum_constructor_to_location_at_offset_with_progress(
                    &field.value,
                    field_type,
                    field_value_layout,
                    destination,
                    nested_offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                    Some(&payload_progress),
                )?
            else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            instructions.append(&mut payload_instructions);
        } else if let (AbiType::Outcome { .. }, Some(storage)) =
            (field_type, outcome_fields.get(field.name.as_str()))
        {
            let Some(mut outcome_instructions) = lower_outcome_field_to_location(
                storage,
                &field.value,
                destination,
                nested_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
            )?
            else {
                return Err(unsupported_aggregate_struct_literal_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            };
            instructions.append(&mut outcome_instructions);
        } else {
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
        if let Some(completed) = progress.and_then(|progress| progress.complete_field(field_offset))
        {
            instructions.push(completed);
        }
    }
    Ok(instructions)
}

pub(super) fn lower_aggregate_call_field_value_to_location(
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

pub(super) fn lower_aggregate_fallible_call_field_value_to_location(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_fallible_call_field_value_to_location_with(
        call,
        expected_layout,
        destination,
        destination_offset,
        diagnostic_code,
        subject,
        context,
        temporaries,
        |_, _, _| Ok(failure_mode),
    )
}

pub(super) fn lower_aggregate_fallible_call_field_value_to_location_with<F>(
    call: &CallExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode_for: F,
) -> Result<Vec<Instruction>, Vec<Diagnostic>>
where
    F: FnOnce(
        AggregateLocation,
        &Type,
        &LoweringContext,
    ) -> Result<OutcomeFailureMode, Vec<Diagnostic>>,
{
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some((_, success_type)) = context
        .call_return_type(&target)
        .and_then(Type::single_outcome)
    else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    let Some(layout) = aggregate_type_layout(success_type) else {
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
    let source = AggregateLocation::Slot(source_slot);
    let failure_mode = failure_mode_for(source, success_type, context)?;
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
        success_type,
        source,
        target,
        arguments,
        layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset,
        source,
        source_offset: 0,
        layout,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_member_field_value_to_location(
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

pub(super) fn call_expression(expression: &Expr) -> Option<&CallExpr> {
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
