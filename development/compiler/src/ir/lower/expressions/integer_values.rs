use super::*;

pub(in crate::ir::lower) fn lower_i32_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_i32_expression_to_location(expression, I32Location::Return, context)
}

pub(in crate::ir::lower) fn lower_i32_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_i32_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_i32_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_i32_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_i32_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                i32_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_i32_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_i32_expression_diagnostic)
        }
        Expr::If(statement) => lower_i32_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_i32_if_is_expression_to_location(statement, destination, context)
        }
        Expr::Match(statement) => lower_i32_match_expression_to_location(
            statement,
            destination,
            context,
            i32_destination_reserved_abi_words(destination),
        ),
        Expr::Unary(unary) if i32_unary_negate_requires_runtime(unary) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_i32_negate_expression_to_location_with_temporaries(
                unary,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            lower_i32_binary_expression_to_location(binary, destination, context)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_i32_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetI32 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::I32) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_i32_conversion_expression_to_value(conversion, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetI32 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_i32_field_to_location(
                expression,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

pub(in crate::ir::lower) fn lower_u8_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_u8_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_u8_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_u8_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_u8_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                u8_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_u8_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_u8_expression_diagnostic)
        }
        Expr::If(statement) => lower_u8_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_u8_if_is_expression_to_location(statement, destination, context)
        }
        Expr::Match(statement) => lower_u8_match_expression_to_location(
            statement,
            destination,
            context,
            u8_destination_reserved_abi_words(destination),
        ),
        Expr::Binary(binary) if is_u8_binary_operator(binary.operator) => {
            lower_u8_binary_expression_to_location(binary, destination, context)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_u8_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetU8 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_u8_expression_to_value(&conversion.expression, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetU8 {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(member) => {
            if let Some(tag) = context.payloadless_enum_variant_tag(member) {
                return Ok(vec![Instruction::SetU8 {
                    destination,
                    value: U8Value::Const(tag),
                }]);
            }
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_u8_field_to_location(expression, destination, context, &mut temporaries)
        }
        Expr::Group(group) => {
            lower_u8_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_u8_value(expression, context)
            .map(|value| vec![Instruction::SetU8 { destination, value }]),
    }
}

pub(in crate::ir::lower) fn lower_usize_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) = lower_builtin_len_call_to_value(call, context, &mut temporaries) {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination,
                    value: lowered.value,
                });
                return Ok(instructions);
            }
            if primitive_addr_call(call, context) {
                return lower_addr_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            if context.primitive_name_for_call(call) == Some("from_addr") {
                let (mut instructions, value) = lower_pointer_address_expression_to_word(
                    expression,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }
            if primitive_from_ref_call(call, context) {
                return lower_from_ref_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            if primitive_pointee_layout_call(call, context) {
                let (mut instructions, value) =
                    lower_pointee_layout_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }
            if primitive_arg_count_raw_call(call, context)
                || primitive_env_count_raw_call(call, context)
            {
                let (mut instructions, value) = if primitive_arg_count_raw_call(call, context) {
                    lower_arg_count_raw_primitive_call_to_word(call)?
                } else {
                    lower_env_count_raw_primitive_call_to_word(call)?
                };
                instructions.push(Instruction::SetUsize { destination, value });
                return Ok(instructions);
            }

            lower_usize_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_usize_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_usize_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_usize_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                usize_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_usize_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_usize_expression_diagnostic)
        }
        Expr::If(statement) => {
            lower_usize_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            lower_usize_if_is_expression_to_location(statement, destination, context)
        }
        Expr::Match(statement) => lower_usize_match_expression_to_location(
            statement,
            destination,
            context,
            usize_destination_reserved_abi_words(destination),
        ),
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            lower_usize_binary_expression_to_location(binary, destination, context)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_usize_index_expression_to_value(index, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetUsize {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::Usize) =>
        {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered =
                lower_usize_conversion_expression_to_value(conversion, context, &mut temporaries)?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetUsize {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_usize_field_to_location(
                expression,
                destination,
                context,
                &mut temporaries,
            )
        }
        Expr::Group(group) => {
            lower_usize_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_usize_value(expression, context)
            .map(|value| vec![Instruction::SetUsize { destination, value }]),
    }
}

pub(in crate::ir::lower) fn lower_i32_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, I32Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_i32_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(in crate::ir::lower) fn lower_i32_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, I32Value), Vec<Diagnostic>> {
    let lowered = lower_i32_expression_to_value(expression, context, temporaries)?;
    Ok((lowered.instructions, lowered.value))
}

pub(in crate::ir::lower) fn lower_u8_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, U8Value), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_u8_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(in crate::ir::lower) fn lower_u8_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, U8Value), Vec<Diagnostic>> {
    let lowered = lower_u8_expression_to_value(expression, context, temporaries)?;
    Ok((lowered.instructions, lowered.value))
}

pub(in crate::ir::lower) fn lower_i32_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .i32_location(&identifier.name)
            .map(I32Value::Location)
            .ok_or_else(unsupported_i32_expression_diagnostic),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            lower_i32_value(&unary.operand, context)
        }
        Expr::Group(group) => lower_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).map(I32Value::Const),
    }
}

pub(in crate::ir::lower) fn lower_u8_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<U8Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .u8_location(&identifier.name)
            .map(U8Value::Location)
            .ok_or_else(unsupported_u8_expression_diagnostic),
        Expr::Member(member) => context
            .payloadless_enum_variant_tag(member)
            .map(U8Value::Const)
            .ok_or_else(unsupported_u8_expression_diagnostic),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            lower_u8_value(&unary.operand, context)
        }
        Expr::Group(group) => lower_u8_value(&group.expression, context),
        _ => lower_u8_literal(expression).map(U8Value::Const),
    }
}

pub(in crate::ir::lower) fn lower_usize_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .usize_location(&identifier.name)
            .map(UsizeValue::Location)
            .ok_or_else(unsupported_usize_expression_diagnostic),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            lower_usize_value(&unary.operand, context)
        }
        Expr::Group(group) => lower_usize_value(&group.expression, context),
        _ => lower_usize_literal(expression).map(UsizeValue::Const),
    }
}

pub(in crate::ir::lower) fn lower_usize_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_usize_expression_to_word_with_temporaries(expression, context, &mut temporaries)
}

pub(in crate::ir::lower) fn lower_usize_expression_to_word_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let lowered = lower_usize_expression_to_value(expression, context, temporaries)?;
    Ok((lowered.instructions, lowered.value))
}
