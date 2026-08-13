use super::*;

pub(super) fn replace_success_returns(instructions: Vec<Instruction>) -> Vec<Instruction> {
    instructions
        .into_iter()
        .map(|instruction| match instruction {
            Instruction::Return => Instruction::ReturnOutcomeSuccess,
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => Instruction::If {
                condition,
                then_instructions: replace_success_returns(then_instructions),
                else_instructions: replace_success_returns(else_instructions),
            },
            instruction => instruction,
        })
        .collect()
}

pub(super) fn never_tail_call_argument_requires_current_frame(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::Borrow(_)) || is_tail_call_stack_pointer_argument(argument)
}

pub(in crate::ir::lower) fn lower_i32_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_take_value_at_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_take_value_at_ptr_primitive_call(
                    call,
                    PointerTakeDestination::I32(I32Location::Return),
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_i32_return_expression(&group.expression, context),
        _ => {
            let mut instructions = lower_i32_expression(expression, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}
pub(in crate::ir::lower) fn success_return_instruction(return_type: &Type) -> Instruction {
    if matches!(
        return_type,
        Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. }
    ) {
        Instruction::ReturnOutcomeSuccess
    } else {
        Instruction::Return
    }
}

pub(in crate::ir::lower) fn mark_outcome_success_returns(
    return_type: &Type,
    instructions: Vec<Instruction>,
) -> Vec<Instruction> {
    if !matches!(
        return_type,
        Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. }
    ) {
        return instructions;
    }

    replace_success_returns(instructions)
}

pub(in crate::ir::lower) fn lower_u8_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_take_value_at_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_take_value_at_ptr_primitive_call(
                    call,
                    PointerTakeDestination::U8(U8Location::Return),
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_u8_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_u8_expression_to_location(expression, U8Location::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_never_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) if primitive_trap_call(call, context) => Ok(Some(vec![Instruction::Trap])),
        Expr::Call(call) if primitive_exit_raw_call(call, context) => {
            lower_exit_raw_primitive_call(call, context).map(Some)
        }
        Expr::Call(call) => {
            let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };
            if context.call_return_type(&target) != Some(&Type::Never) {
                return Ok(None);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, arguments) =
                lower_call_arguments(call, &target, &call_name, context, &mut temporaries)?;
            let requires_current_frame = arguments
                .iter()
                .any(never_tail_call_argument_requires_current_frame);
            if requires_current_frame || call_arguments_require_stack(&arguments, &call_name)? {
                instructions.push(Instruction::CallVoid { target, arguments });
                instructions.push(Instruction::Trap);
                return Ok(Some(instructions));
            }
            instructions.push(Instruction::TailCall { target, arguments });
            Ok(Some(instructions))
        }
        Expr::Group(group) => lower_never_return_expression(&group.expression, context),
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_usize_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) = lower_literal_pack_len_call_to_value(call, context) {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: lowered.value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_view_len_call(call, context) {
                let lowered =
                    lower_view_len_primitive_call_to_value(call, context, &mut temporaries)?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: lowered.value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_view_pointer_call(call, context) {
                let lowered =
                    lower_view_pointer_primitive_call_to_value(call, context, &mut temporaries)?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: lowered.value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_take_value_at_ptr_call(call, context) {
                let mut instructions = lower_take_value_at_ptr_primitive_call(
                    call,
                    PointerTakeDestination::Usize(UsizeLocation::Return),
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_addr_call(call, context) {
                let mut instructions = lower_addr_primitive_call_to_location(
                    call,
                    UsizeLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if context.intrinsic_for_call(call) == Some(crate::intrinsics::IntrinsicId::FromAddr) {
                let (mut instructions, value) = lower_pointer_address_expression_to_word(
                    expression,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_from_ref_call(call, context) {
                let mut instructions = lower_from_ref_primitive_call_to_location(
                    call,
                    UsizeLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_pointee_layout_call(call, context) {
                let (mut instructions, value) =
                    lower_pointee_layout_primitive_call_to_word(call, context, &mut temporaries)?;
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
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
                instructions.push(Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }

            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_usize_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_usize_expression_to_location(expression, UsizeLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_str_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_take_value_at_ptr_call(call, context) {
                let mut instructions = lower_take_value_at_ptr_primitive_call(
                    call,
                    PointerTakeDestination::Str(StrLocation::Return),
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_arg_raw_call(call, context) || primitive_env_entry_raw_call(call, context)
            {
                let (mut instructions, value) = if primitive_arg_raw_call(call, context) {
                    lower_arg_raw_primitive_call_to_value(call, context, &mut temporaries)?
                } else {
                    lower_env_entry_raw_primitive_call_to_value(call, context, &mut temporaries)?
                };
                instructions.push(Instruction::SetStr {
                    destination: StrLocation::Return,
                    value,
                });
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_str_from_raw_parts_call(call, context) {
                let mut instructions = lower_str_from_raw_parts_primitive_call_to_location(
                    call,
                    StrLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_str_subview_call(call, context) {
                let mut instructions = lower_str_subview_primitive_call_to_location(
                    call,
                    StrLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_str_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_str_expression_to_location(expression, StrLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_slice_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_slice_from_raw_parts_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    SliceLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            if primitive_bytes_from_str_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                let mut instructions = lower_str_bytes_primitive_call_to_location(
                    call,
                    SliceLocation::Return,
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => lower_slice_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_slice_expression_to_location(expression, SliceLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_bool_return_expression(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_take_value_at_ptr_call(call, context) {
                let mut instructions = lower_take_value_at_ptr_primitive_call(
                    call,
                    PointerTakeDestination::Bool(BoolLocation::Return),
                    context,
                    &mut temporaries,
                )?;
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
            lower_direct_tail_call(call, context)
        }
        Expr::Group(group) => {
            lower_bool_return_expression(&group.expression, context, diagnostic_code)
        }
        _ => {
            let mut instructions = lower_bool_expression_to_location(
                expression,
                BoolLocation::Return,
                context,
                diagnostic_code,
            )?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}
