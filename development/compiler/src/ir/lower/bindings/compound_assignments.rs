use super::*;

pub(super) fn lower_compound_assignment(
    statement: &AssignmentStmt,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(&statement.target) {
        Expr::Identifier(identifier) => {
            lower_compound_identifier_assignment(statement, identifier, context)
        }
        Expr::Member(member) => {
            lower_compound_aggregate_field_assignment(statement, member, context)
        }
        Expr::Index(index) => lower_compound_index_assignment(statement, index, context),
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_compound_identifier_assignment(
    statement: &AssignmentStmt,
    identifier: &crate::ast::IdentifierExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(destination) = context.i32_location(&identifier.name) {
        let I32Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_i32_expression_to_word(&statement.value, context)?;
        instructions.push(i32_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    if let Some(destination) = context.usize_location(&identifier.name) {
        let UsizeLocation::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_usize_expression_to_word(&statement.value, context)?;
        instructions.push(usize_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    if let Some(destination) = context.u8_location(&identifier.name) {
        let U8Location::Local(_) = destination else {
            return Err(unsupported_assignment_diagnostic());
        };
        let (mut instructions, right) = lower_u8_expression_to_word(&statement.value, context)?;
        instructions.push(u8_compound_assignment_instruction(
            statement.operator,
            destination,
            right,
        )?);
        return Ok(instructions);
    }

    Err(unsupported_assignment_diagnostic())
}

pub(super) fn lower_compound_aggregate_field_assignment(
    statement: &AssignmentStmt,
    target: &MemberExpr,
    context: &LoweringContext,
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
    match field.kind {
        AggregateFieldKind::I32 => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::LoadAggregateI32 {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateI32 {
                destination: field.source,
                offset: field.offset,
                value: I32Value::Location(current),
            });
            Ok(instructions)
        }
        AggregateFieldKind::Usize => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateUsize {
                destination: field.source,
                offset: field.offset,
                value: UsizeValue::Location(current),
            });
            Ok(instructions)
        }
        AggregateFieldKind::U8 => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::LoadAggregateU8 {
                destination: current,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateU8 {
                destination: field.source,
                offset: field.offset,
                value: U8Value::Location(current),
            });
            Ok(instructions)
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_compound_index_assignment(
    statement: &AssignmentStmt,
    target: &IndexExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) =
        lower_fixed_array_index_compound_assignment(statement, target, context)?
    {
        return Ok(instructions);
    }
    if let Some(instructions) =
        lower_fixed_array_indexed_compound_assignment(statement, target, context)?
    {
        return Ok(instructions);
    }
    lower_compound_slice_index_assignment(statement, target, context)
}

pub(super) fn lower_fixed_array_index_compound_assignment(
    statement: &AssignmentStmt,
    target: &IndexExpr,
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
            let (value_instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
                return Ok(Some(instructions));
            }
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::LoadAggregateI32 {
                destination: current,
                source: access.source,
                offset: access.offset,
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateI32 {
                destination: access.source,
                offset: access.offset,
                value: I32Value::Location(current),
            });
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let (value_instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
                return Ok(Some(instructions));
            }
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::LoadAggregateU8 {
                destination: current,
                source: access.source,
                offset: access.offset,
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateU8 {
                destination: access.source,
                offset: access.offset,
                value: U8Value::Location(current),
            });
            Ok(Some(instructions))
        }
        AbiType::Usize => {
            let (value_instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            let mut instructions = access.instructions;
            instructions.extend(value_instructions);
            if access.out_of_bounds {
                instructions.push(Instruction::Trap);
                return Ok(Some(instructions));
            }
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: current,
                source: access.source,
                offset: access.offset,
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateUsize {
                destination: access.source,
                offset: access.offset,
                value: UsizeValue::Location(current),
            });
            Ok(Some(instructions))
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_fixed_array_indexed_compound_assignment(
    statement: &AssignmentStmt,
    target: &IndexExpr,
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
            let (value_instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::LoadAggregateI32Indexed {
                destination: current,
                source: access.source,
                base_offset: access.base_offset,
                index: index.clone(),
                length: access.length,
                stride: access.stride,
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateI32Indexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value: I32Value::Location(current),
            });
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let (value_instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::LoadAggregateU8Indexed {
                destination: current,
                source: access.source,
                base_offset: access.base_offset,
                index: index.clone(),
                length: access.length,
                stride: access.stride,
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateU8Indexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value: U8Value::Location(current),
            });
            Ok(Some(instructions))
        }
        AbiType::Usize => {
            let (value_instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsizeIndexed {
                destination: current,
                source: access.source,
                base_offset: access.base_offset,
                index: index.clone(),
                length: access.length,
                stride: access.stride,
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreAggregateUsizeIndexed {
                destination: access.source,
                base_offset: access.base_offset,
                index,
                length: access.length,
                stride: access.stride,
                value: UsizeValue::Location(current),
            });
            Ok(Some(instructions))
        }
        _ => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn lower_compound_slice_index_assignment(
    statement: &AssignmentStmt,
    target: &IndexExpr,
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
        TypecheckSliceElementKind::I32 => {
            let (value_instructions, right) = lower_i32_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_i32()?;
            instructions.push(Instruction::SetI32 {
                destination: current,
                value: I32Value::SliceIndex {
                    source: destination,
                    index: index.clone(),
                },
            });
            instructions.push(i32_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value: I32Value::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Usize => {
            let (value_instructions, right) = lower_usize_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize {
                destination: current,
                value: UsizeValue::SliceIndex {
                    source: destination,
                    index: Box::new(index.clone()),
                },
            });
            instructions.push(usize_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value: UsizeValue::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::U8 => {
            let (value_instructions, right) = lower_u8_expression_to_word_with_temporaries(
                &statement.value,
                context,
                &mut temporaries,
            )?;
            instructions.extend(value_instructions);
            let current = temporaries.next_u8()?;
            instructions.push(Instruction::SetU8 {
                destination: current,
                value: U8Value::SliceIndex {
                    source: destination,
                    index: index.clone(),
                },
            });
            instructions.push(u8_compound_assignment_instruction(
                statement.operator,
                current,
                right,
            )?);
            instructions.push(Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value: U8Value::Location(current),
            });
            Ok(instructions)
        }
        TypecheckSliceElementKind::Bool
        | TypecheckSliceElementKind::Str
        | TypecheckSliceElementKind::Other => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn i32_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: I32Location,
    right: I32Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = I32Value::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderI32 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn usize_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: UsizeLocation,
    right: UsizeValue,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = UsizeValue::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderUsize {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
    }
}

pub(super) fn u8_compound_assignment_instruction(
    operator: AssignmentOperator,
    destination: U8Location,
    right: U8Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    let left = U8Value::Location(destination);
    match operator {
        AssignmentOperator::AddAssign => Ok(Instruction::AddU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::SubtractAssign => Ok(Instruction::SubtractU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::MultiplyAssign => Ok(Instruction::MultiplyU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::DivideAssign => Ok(Instruction::DivideU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::RemainderAssign => Ok(Instruction::RemainderU8 {
            destination,
            left,
            right,
        }),
        AssignmentOperator::Assign => Err(unsupported_assignment_diagnostic()),
    }
}
