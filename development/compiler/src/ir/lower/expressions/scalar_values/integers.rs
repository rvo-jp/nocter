use super::*;

pub(in crate::ir::lower::expressions) fn lower_i32_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_i32_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower::expressions) fn lower_i32_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_i32_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_i32_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(i32_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_i32_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_normal_call(call, temporary, context, temporaries)?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        i32_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_i32()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredI32Value {
                instructions: lower_i32_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_i32_expression_diagnostic)?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::If(statement) => lower_i32_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_i32_if_is_expression_to_value(statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_i32_match_expression_to_value(statement, context, temporaries)
        }
        Expr::Unary(unary) if i32_unary_negate_requires_runtime(unary) => {
            let destination = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_negate_expression_to_location_with_temporaries(
                    unary,
                    destination,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(destination),
            })
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Index(index) => lower_i32_index_expression_to_value(index, context, temporaries),
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::I32) =>
        {
            lower_i32_conversion_expression_to_value(conversion, context, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_aggregate_i32_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: I32Value::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_i32_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredI32Value {
            instructions: Vec::new(),
            value: lower_i32_value(expression, context)?,
        }),
    }
}

pub(in crate::ir::lower::expressions) fn lower_i32_negate_expression_to_location_with_temporaries(
    unary: &UnaryExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let right = lower_i32_expression_to_value(&unary.operand, context, temporaries)?;
    let mut instructions = right.instructions;
    instructions.push(Instruction::SubtractI32 {
        destination,
        left: I32Value::Const(0),
        right: right.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn i32_unary_negate_requires_runtime(
    unary: &UnaryExpr,
) -> bool {
    unary.operator == UnaryOperator::Negate
        && !expression_is_unsigned_integer_literal(&unary.operand)
}

pub(in crate::ir::lower::expressions) fn expression_is_unsigned_integer_literal(
    expression: &Expr,
) -> bool {
    match expression {
        Expr::IntegerLiteral(_) => true,
        Expr::Group(group) => expression_is_unsigned_integer_literal(&group.expression),
        _ => false,
    }
}

pub(in crate::ir::lower::expressions) fn lower_u8_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_normal_call(call, temporary, context, temporaries)?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        u8_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_u8()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredU8Value {
                instructions: lower_u8_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_u8_expression_diagnostic)?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::If(statement) => lower_u8_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_u8_if_is_expression_to_value(statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_u8_match_expression_to_value(statement, context, temporaries)
        }
        Expr::Binary(binary) if is_u8_binary_operator(binary.operator) => {
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_u8_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Index(index) => lower_u8_index_expression_to_value(index, context, temporaries),
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            lower_u8_expression_to_value(&conversion.expression, context, temporaries)
        }
        Expr::Member(member) => {
            if let Some(tag) = context.payloadless_enum_variant_tag(member) {
                return Ok(LoweredU8Value {
                    instructions: Vec::new(),
                    value: U8Value::Const(tag),
                });
            }
            let temporary = temporaries.next_u8()?;
            Ok(LoweredU8Value {
                instructions: lower_aggregate_u8_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: U8Value::Location(temporary),
            })
        }
        Expr::Group(group) => lower_u8_expression_to_value(&group.expression, context, temporaries),
        _ => Ok(LoweredU8Value {
            instructions: Vec::new(),
            value: lower_u8_value(expression, context)?,
        }),
    }
}

pub(in crate::ir::lower::expressions) fn lower_u8_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_u8_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower::expressions) fn lower_u8_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_u8_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_u8_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(u8_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_i32_conversion_expression_to_value(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    if let Ok(value) = lower_i32_value(&conversion.expression, context) {
        return Ok(LoweredI32Value {
            instructions: Vec::new(),
            value,
        });
    }

    let value = lower_u8_expression_to_value(&conversion.expression, context, temporaries)?;
    Ok(LoweredI32Value {
        instructions: value.instructions,
        value: I32Value::U8ZeroExtend(Box::new(value.value)),
    })
}

pub(in crate::ir::lower::expressions) fn lower_usize_conversion_expression_to_value(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if let Ok(value) = lower_usize_value(&conversion.expression, context) {
        return Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value,
        });
    }

    let value = lower_u8_expression_to_value(&conversion.expression, context, temporaries)?;
    Ok(LoweredUsizeValue {
        instructions: value.instructions,
        value: UsizeValue::U8ZeroExtend(Box::new(value.value)),
    })
}

pub(in crate::ir::lower::expressions) fn type_conversion_target_is(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    expected: Type,
) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    scalar_or_view_type_from_type_expr(&conversion.ty, resolved) == Some(expected)
}

pub(in crate::ir::lower::expressions) fn lower_usize_binary_expression_to_location(
    binary: &BinaryExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_usize_binary_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower::expressions) fn lower_usize_binary_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_usize_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_usize_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(usize_binary_instruction(
        binary.operator,
        destination,
        left.value,
        right.value,
    )?);
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_usize_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if let Some(value) = lower_builtin_len_call_to_value(call, context, temporaries) {
                return value;
            }
            if primitive_addr_call(call, context) {
                let (instructions, value) =
                    lower_addr_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if context.primitive_name_for_call(call) == Some("from_addr") {
                let (instructions, value) =
                    lower_pointer_address_expression_to_word(expression, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_from_ref_call(call, context) {
                let (instructions, value) =
                    lower_from_ref_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_pointee_size_call(call, context) {
                let (instructions, value) =
                    lower_pointee_size_primitive_call_to_word(call, context, temporaries)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }
            if primitive_arg_count_raw_call(call, context) {
                let (instructions, value) = lower_arg_count_raw_primitive_call_to_word(call)?;
                return Ok(LoweredUsizeValue {
                    instructions,
                    value,
                });
            }

            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_normal_call(call, temporary, context, temporaries)?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_failure_mode(context)?,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    FallibleFailureMode::Trap,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        usize_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_usize()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredUsizeValue {
                instructions: lower_usize_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_usize_expression_diagnostic)?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::If(statement) => lower_usize_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_usize_if_is_expression_to_value(statement, context, temporaries)
        }
        Expr::Match(statement) => {
            lower_usize_match_expression_to_value(statement, context, temporaries)
        }
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_binary_expression_to_location_with_temporaries(
                    binary,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Index(index) => lower_usize_index_expression_to_value(index, context, temporaries),
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::Usize) =>
        {
            lower_usize_conversion_expression_to_value(conversion, context, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_aggregate_usize_field_to_location(
                    expression,
                    temporary,
                    context,
                    temporaries,
                )?,
                value: UsizeValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_usize_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value: lower_usize_value(expression, context)?,
        }),
    }
}

pub(in crate::ir::lower::expressions) fn i32_binary_instruction(
    operator: BinaryOperator,
    destination: I32Location,
    left: I32Value,
    right: I32Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftI32 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightI32 {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_i32_expression_diagnostic()),
    }
}

pub(in crate::ir::lower::expressions) fn usize_binary_instruction(
    operator: BinaryOperator,
    destination: UsizeLocation,
    left: UsizeValue,
    right: UsizeValue,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftUsize {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightUsize {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_usize_expression_diagnostic()),
    }
}

pub(in crate::ir::lower::expressions) fn u8_binary_instruction(
    operator: BinaryOperator,
    destination: U8Location,
    left: U8Value,
    right: U8Value,
) -> Result<Instruction, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Add => Ok(Instruction::AddU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Subtract => Ok(Instruction::SubtractU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Multiply => Ok(Instruction::MultiplyU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Divide => Ok(Instruction::DivideU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::Remainder => Ok(Instruction::RemainderU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftLeft => Ok(Instruction::ShiftLeftU8 {
            destination,
            left,
            right,
        }),
        BinaryOperator::ShiftRight => Ok(Instruction::ShiftRightU8 {
            destination,
            left,
            right,
        }),
        _ => Err(unsupported_u8_expression_diagnostic()),
    }
}
