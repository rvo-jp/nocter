use super::*;

pub(super) fn lower_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_fixed_array_index_assignment(target, value, context)? {
        return Ok(instructions);
    }
    if let Some(instructions) = lower_fixed_array_indexed_assignment(target, value, context)? {
        return Ok(instructions);
    }
    lower_slice_index_assignment(target, value, context)
}

pub(super) fn lower_fixed_array_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let Some(access) = fixed_array_element_access(
        target,
        context,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if !access.is_readwrite {
        return Err(unsupported_assignment_diagnostic());
    }

    match access.element {
        AbiType::I32 => {
            let (value_instructions, value) =
                lower_i32_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateI32 {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let (value_instructions, value) =
                lower_u8_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateU8 {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::Usize => {
            let (value_instructions, value) =
                lower_usize_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateUsize {
                    destination: access.source,
                    offset: access.offset,
                    value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::Bool => {
            let mut lowered = lower_bool_expression_to_value_with_temporaries(
                value,
                context,
                "E8008",
                &mut temporaries,
            )?;
            let mut instructions = access.instructions;
            instructions.append(&mut lowered.instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                instructions.push(Instruction::StoreAggregateBool {
                    destination: access.source,
                    offset: access.offset,
                    value: lowered.value,
                });
            }
            Ok(Some(instructions))
        }
        AbiType::StrView => {
            let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
            let mut instructions = access.instructions;
            instructions.append(&mut lowered.instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
            } else {
                push_store_str_view_to_aggregate_field(
                    &mut instructions,
                    access.source,
                    access.offset,
                    lowered.value,
                    &mut temporaries,
                    unsupported_assignment_diagnostic,
                )?;
            }
            Ok(Some(instructions))
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_fixed_array_indexed_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let Some(access) = fixed_array_element_indexed_access(
        target,
        context,
        &mut temporaries,
        unsupported_assignment_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if !access.is_readwrite {
        return Err(unsupported_assignment_diagnostic());
    }
    let mut instructions = access.index_instructions;
    let index = materialize_slice_index_assignment_index(
        &mut instructions,
        access.index,
        &mut temporaries,
    )?;

    match access.element {
        AbiType::I32 => {
            let (value_instructions, value) =
                lower_i32_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreAggregateI32Indexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value,
            });
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let (value_instructions, value) =
                lower_u8_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreAggregateU8Indexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value,
            });
            Ok(Some(instructions))
        }
        AbiType::Usize => {
            let (value_instructions, value) =
                lower_usize_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreAggregateUsizeIndexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value,
            });
            Ok(Some(instructions))
        }
        AbiType::Bool => {
            let mut lowered = lower_bool_expression_to_value_with_temporaries(
                value,
                context,
                "E8008",
                &mut temporaries,
            )?;
            instructions.append(&mut lowered.instructions);
            instructions.push(Instruction::StoreAggregateBoolIndexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value: lowered.value,
            });
            Ok(Some(instructions))
        }
        AbiType::StrView => {
            let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
            instructions.append(&mut lowered.instructions);
            push_store_str_view_to_fixed_array_indexed_element(
                &mut instructions,
                access.source,
                access.base_offset,
                index,
                access.length,
                access.stride,
                lowered.value,
                &mut temporaries,
            )?;
            Ok(Some(instructions))
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn push_store_str_view_to_fixed_array_indexed_element(
    instructions: &mut Vec<Instruction>,
    destination: AggregateLocation,
    base_offset: u32,
    index: UsizeValue,
    length: u64,
    stride: u32,
    value: StrValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<(), Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let StrLocation::Local(local_index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = local_index
        .checked_add(1)
        .ok_or_else(unsupported_assignment_diagnostic)?;
    let len_base_offset = base_offset
        .checked_add(8)
        .ok_or_else(unsupported_assignment_diagnostic)?;

    instructions.push(Instruction::SetStr {
        destination: temporary,
        value,
    });
    instructions.push(Instruction::StoreAggregateUsizeIndexed {
        destination,
        base_offset,
        index: index.clone(),
        length,
        stride,
        value: UsizeValue::Location(UsizeLocation::Local(local_index)),
    });
    instructions.push(Instruction::StoreAggregateUsizeIndexed {
        destination,
        base_offset: len_base_offset,
        index,
        length,
        stride,
        value: UsizeValue::Location(UsizeLocation::Local(len_index)),
    });
    Ok(())
}

pub(super) fn lower_slice_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let element_kind = slice_index_assignment_element_kind(&target.object, context);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered_slice = lower_slice_expression_to_value(&target.object, context, &mut temporaries)?;
    let SliceValue::Location(destination) = lowered_slice.value else {
        return Err(unsupported_assignment_diagnostic());
    };
    let (index_instructions, index) =
        lower_usize_expression_to_word_with_temporaries(&target.index, context, &mut temporaries)?;
    let mut instructions = lowered_slice.instructions;
    instructions.extend(index_instructions);
    let index =
        materialize_slice_index_assignment_index(&mut instructions, index, &mut temporaries)?;

    match element_kind {
        TypecheckSliceElementKind::U8 => {
            let (value_instructions, value) =
                lower_u8_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::I32 => {
            let (value_instructions, value) =
                lower_i32_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Usize => {
            let (value_instructions, value) =
                lower_usize_expression_to_word_with_temporaries(value, context, &mut temporaries)?;
            instructions.extend(value_instructions);
            instructions.push(Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Bool => {
            let mut lowered = lower_bool_expression_to_value_with_temporaries(
                value,
                context,
                "E8008",
                &mut temporaries,
            )?;
            instructions.append(&mut lowered.instructions);
            instructions.push(Instruction::StoreBoolToSliceIndex {
                destination,
                index,
                value: lowered.value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Str => {
            let mut lowered = lower_str_expression_to_value(value, context, &mut temporaries)?;
            instructions.append(&mut lowered.instructions);
            instructions.push(Instruction::StoreStrToSliceIndex {
                destination,
                index,
                value: lowered.value,
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Other => lower_copy_aggregate_slice_index_assignment(
            target,
            value,
            destination,
            index,
            instructions,
            context,
            &mut temporaries,
        ),
    }
}

pub(super) fn lower_copy_aggregate_slice_index_assignment(
    target: &IndexExpr,
    value: &Expr,
    destination: SliceLocation,
    index: UsizeValue,
    mut instructions: Vec<Instruction>,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(element) = copy_aggregate_slice_index_element(target, context) else {
        return Err(unsupported_assignment_diagnostic());
    };
    let source_slot = temporaries.next_aggregate_slot();
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: source_slot,
        layout: element.layout,
    });
    instructions.extend(lower_copy_aggregate_value_to_slot_with_temporaries(
        source_slot,
        element.layout,
        value,
        context,
        temporaries,
    )?);
    let index = materialize_slice_aggregate_index(&mut instructions, index, temporaries)?;
    instructions.push(Instruction::CopyAggregateToSliceElement {
        destination,
        index,
        source: AggregateLocation::Slot(source_slot),
        layout: element.layout,
    });
    Ok(instructions)
}

pub(super) fn lower_copy_aggregate_value_to_slot_with_temporaries(
    slot_index: usize,
    layout: ValueLayout,
    value: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(value) {
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_to_location_with_temporaries(
                literal,
                layout,
                AggregateLocation::Slot(slot_index),
                "E8008",
                "slice index assignments",
                context
                    .resolved_calls()
                    .map(|(_root_source, resolved)| resolved)
                    .ok_or_else(unsupported_assignment_diagnostic)?,
                context,
                temporaries,
            )
        }
        Expr::Identifier(identifier) => {
            lower_aggregate_copy_assignment(slot_index, layout, &identifier.name, context)
        }
        Expr::Member(_) => lower_aggregate_member_value_assignment(
            AggregateLocation::Slot(slot_index),
            0,
            layout,
            value,
            context,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
                return Err(unsupported_assignment_diagnostic());
            };
            lower_aggregate_move_assignment(slot_index, layout, &identifier.name, context)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn materialize_slice_index_assignment_index(
    instructions: &mut Vec<Instruction>,
    value: UsizeValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match value {
        UsizeValue::Const(_) | UsizeValue::Location(_) => Ok(value),
        _ => {
            let destination = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize { destination, value });
            Ok(UsizeValue::Location(destination))
        }
    }
}

pub(super) fn materialize_slice_aggregate_index(
    instructions: &mut Vec<Instruction>,
    value: UsizeValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<SliceElementIndex, Vec<Diagnostic>> {
    match value {
        UsizeValue::Const(value) => Ok(SliceElementIndex::Const(value)),
        UsizeValue::Location(location) => Ok(SliceElementIndex::Location(location)),
        value => {
            let destination = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize { destination, value });
            Ok(SliceElementIndex::Location(destination))
        }
    }
}

pub(super) fn slice_index_assignment_element_kind(
    object: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    match unwrap_group(object) {
        Expr::Identifier(identifier) => context
            .slice_element_kind(&identifier.name)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Call(call) => call_return_slice_element_kind(call, context)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Member(member) => match aggregate_member_field_kind_from_member(member, context)
            .ok()
            .flatten()
        {
            Some(AggregateFieldKind::Slice(info)) => info.element_kind,
            _ => TypecheckSliceElementKind::Other,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_element_kind(
            unwrap_group(&propagation.expression),
            context,
        ),
        Expr::Force(force) => {
            slice_index_assignment_fallible_element_kind(unwrap_group(&force.expression), context)
        }
        Expr::Catch(catch) => {
            slice_index_assignment_fallible_element_kind(unwrap_group(&catch.expression), context)
        }
        Expr::Group(group) => slice_index_assignment_element_kind(&group.expression, context),
        _ => TypecheckSliceElementKind::Other,
    }
}

pub(super) fn slice_index_assignment_fallible_element_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Expr::Call(call) = expression else {
        return TypecheckSliceElementKind::Other;
    };
    call_success_slice_element_kind(call, context).unwrap_or(TypecheckSliceElementKind::Other)
}
