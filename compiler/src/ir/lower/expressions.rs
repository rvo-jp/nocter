use super::context::LoweringContext;
use super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
mod calls;
mod predicates;
mod temporaries;

use crate::ast::{
    BinaryExpr, BinaryOperator, Expr, IndexExpr, TypeConversionExpr, TypeExpr, UnaryExpr,
    UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, I32ComparisonOperator,
    I32Location, I32Value, Instruction, SliceLocation, SliceValue, StrLocation, StrValue, Type,
    U8Location, U8Value, UsizeLocation, UsizeValue,
};
use calls::{
    lower_bool_normal_call, lower_call_arguments, lower_direct_tail_call, lower_i32_normal_call,
    lower_slice_normal_call, lower_str_normal_call, lower_u8_normal_call, lower_usize_normal_call,
    primitive_trap_call,
};
use predicates::{
    bool_comparison_contains_call, expressions_are_lowerable_bool_comparison_operands,
    expressions_are_lowerable_bool_values, expressions_are_lowerable_usize_values,
    i32_comparison_needs_temporaries, is_i32_binary_operator, is_usize_binary_operator,
    short_circuit_bool_expression_contains_call, usize_comparison_needs_temporaries,
};
pub(super) use predicates::{
    expression_contains_call, expression_contains_interpolated_string,
    expression_is_lowerable_bool_binding, expression_is_unsupported_bool_comparison_binding,
};
use temporaries::{
    LoweredI32Value, LoweredSliceValue, LoweredStrValue, LoweredU8Value, LoweredUsizeValue,
    TemporaryAllocator,
};

pub(super) fn lower_i32_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_i32_expression_to_location(expression, I32Location::Return, context)
}

pub(super) fn lower_i32_expression_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_i32_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            lower_i32_binary_expression_to_location(binary, destination, context)
        }
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "i32") => {
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
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

pub(super) fn lower_u8_expression_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_u8_normal_call(call, destination, context, &mut temporaries)
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
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "u8") => {
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
        Expr::Group(group) => {
            lower_u8_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_u8_value(expression, context)
            .map(|value| vec![Instruction::SetU8 { destination, value }]),
    }
}

pub(super) fn lower_usize_expression_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if let Some(value) = lower_builtin_len_call_value(call, context) {
                return value.map(|value| vec![Instruction::SetUsize { destination, value }]);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_usize_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            lower_usize_binary_expression_to_location(binary, destination, context)
        }
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "usize") => {
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
        Expr::Group(group) => {
            lower_usize_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_usize_value(expression, context)
            .map(|value| vec![Instruction::SetUsize { destination, value }]),
    }
}

pub(super) fn lower_str_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_str_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Group(group) => {
            lower_str_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_str_value(expression, context)
            .map(|value| vec![Instruction::SetStr { destination, value }]),
    }
}

pub(super) fn lower_slice_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_slice_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Group(group) => {
            lower_slice_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_slice_value(expression, context)
            .map(|value| vec![Instruction::SetSlice { destination, value }]),
    }
}

fn lower_i32_binary_expression_to_location(
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

fn lower_i32_binary_expression_to_location_with_temporaries(
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

fn lower_i32_expression_to_value(
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
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "i32") => {
            lower_i32_conversion_expression_to_value(conversion, context, temporaries)
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

fn lower_u8_expression_to_value(
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
        Expr::Index(index) => lower_u8_index_expression_to_value(index, context, temporaries),
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "u8") => {
            lower_u8_expression_to_value(&conversion.expression, context, temporaries)
        }
        Expr::Group(group) => lower_u8_expression_to_value(&group.expression, context, temporaries),
        _ => Ok(LoweredU8Value {
            instructions: Vec::new(),
            value: lower_u8_value(expression, context)?,
        }),
    }
}

fn lower_i32_conversion_expression_to_value(
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

fn lower_usize_conversion_expression_to_value(
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

fn type_conversion_target_is(conversion: &TypeConversionExpr, name: &str) -> bool {
    matches!(&conversion.ty, TypeExpr::Reference(reference) if reference.name == name)
}

fn lower_usize_binary_expression_to_location(
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

fn lower_usize_binary_expression_to_location_with_temporaries(
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

fn lower_usize_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if let Some(value) = lower_builtin_len_call_value(call, context) {
                return Ok(LoweredUsizeValue {
                    instructions: Vec::new(),
                    value: value?,
                });
            }

            let temporary = temporaries.next_usize()?;
            Ok(LoweredUsizeValue {
                instructions: lower_usize_normal_call(call, temporary, context, temporaries)?,
                value: UsizeValue::Location(temporary),
            })
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
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "usize") => {
            lower_usize_conversion_expression_to_value(conversion, context, temporaries)
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

fn lower_str_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_normal_call(call, temporary, context, temporaries)?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_str_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredStrValue {
            instructions: Vec::new(),
            value: lower_str_value(expression, context)?,
        }),
    }
}

fn lower_slice_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_normal_call(call, temporary, context, temporaries)?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Group(group) => {
            lower_slice_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredSliceValue {
            instructions: Vec::new(),
            value: lower_slice_value(expression, context)?,
        }),
    }
}

fn i32_binary_instruction(
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

fn usize_binary_instruction(
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

pub(super) fn lower_i32_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_i32_return_expression(&group.expression, context),
        _ => {
            let mut instructions = lower_i32_expression(expression, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_u8_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_u8_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_u8_expression_to_location(expression, U8Location::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_never_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) if primitive_trap_call(call, context) => Ok(Some(vec![Instruction::Trap])),
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return Ok(None);
            };
            let target = context.call_target(call, &identifier.name);
            if context.call_return_type(&target) != Some(&Type::Never) {
                return Ok(None);
            }

            let mut temporaries = TemporaryAllocator::new(context)?;
            let (mut instructions, arguments) =
                lower_call_arguments(call, &target, &identifier.name, context, &mut temporaries)?;
            instructions.push(Instruction::TailCall { target, arguments });
            Ok(Some(instructions))
        }
        Expr::Group(group) => lower_never_return_expression(&group.expression, context),
        _ => Ok(None),
    }
}

pub(super) fn lower_usize_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if lower_builtin_len_call_value(call, context).is_some() {
                let mut instructions =
                    lower_usize_expression_to_location(expression, UsizeLocation::Return, context)?;
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

pub(super) fn lower_str_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_str_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_str_expression_to_location(expression, StrLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_slice_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
        Expr::Group(group) => lower_slice_return_expression(&group.expression, context),
        _ => {
            let mut instructions =
                lower_slice_expression_to_location(expression, SliceLocation::Return, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

pub(super) fn lower_bool_return_expression(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_direct_tail_call(call, context),
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

pub(super) fn lower_bool_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Binary(binary) if short_circuit_bool_expression_contains_call(binary) => {
            lower_short_circuit_bool_expression_to_location(
                binary,
                destination,
                context,
                diagnostic_code,
            )
        }
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_bool_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = operand.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: BoolValue::Not(Box::new(operand.value)),
            });
            Ok(instructions)
        }
        Expr::Group(group) => lower_bool_expression_to_location(
            &group.expression,
            destination,
            context,
            diagnostic_code,
        ),
        _ => Ok(vec![Instruction::SetBool {
            destination,
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }]),
    }
}

fn lower_short_circuit_bool_expression_to_location(
    binary: &BinaryExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_expression_to_branch(
        &Expr::Binary(binary.clone()),
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(true),
        }],
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(false),
        }],
        context,
        diagnostic_code,
    )
}

fn lower_bool_expression_to_branch(
    expression: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Expr::Binary(binary) = unwrap_group(expression)
        && short_circuit_bool_expression_contains_call(binary)
    {
        return lower_short_circuit_bool_expression_to_branch(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
        );
    }

    let condition = lower_bool_expression_to_value(expression, context, diagnostic_code)?;
    let mut instructions = condition.instructions;
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions,
        else_instructions,
    });
    Ok(instructions)
}

fn lower_short_circuit_bool_expression_to_branch(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => lower_bool_expression_to_branch(
            &binary.left,
            lower_bool_expression_to_branch(
                &binary.right,
                then_instructions,
                else_instructions.clone(),
                context,
                diagnostic_code,
            )?,
            else_instructions,
            context,
            diagnostic_code,
        ),
        BinaryOperator::LogicalOr => lower_bool_expression_to_branch(
            &binary.left,
            then_instructions.clone(),
            lower_bool_expression_to_branch(
                &binary.right,
                then_instructions,
                else_instructions,
                context,
                diagnostic_code,
            )?,
            context,
            diagnostic_code,
        ),
        _ => unreachable!("short-circuit bool expression must be && or ||"),
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) struct LoweredBoolValue {
    pub(super) instructions: Vec<Instruction>,
    pub(super) value: BoolValue,
}

pub(super) fn lower_bool_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_bool_expression_to_value_with_temporaries(
        expression,
        context,
        diagnostic_code,
        &mut temporaries,
    )
}

fn lower_bool_expression_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if usize_comparison_needs_temporaries(binary, context) => {
            lower_usize_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Call(call) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_normal_call(call, temporary, context, temporaries)?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                temporaries,
            )?;
            Ok(LoweredBoolValue {
                instructions: operand.instructions,
                value: BoolValue::Not(Box::new(operand.value)),
            })
        }
        Expr::Group(group) => lower_bool_expression_to_value_with_temporaries(
            &group.expression,
            context,
            diagnostic_code,
            temporaries,
        ),
        _ => Ok(LoweredBoolValue {
            instructions: Vec::new(),
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }),
    }
}

fn lower_bool_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::Equal => BoolComparisonOperator::Equal,
        BinaryOperator::NotEqual => BoolComparisonOperator::NotEqual,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    let left = lower_bool_comparison_operand_to_value_with_temporaries(
        &binary.left,
        context,
        diagnostic_code,
        temporaries,
    )?;
    let right = lower_bool_comparison_operand_to_value_with_temporaries(
        &binary.right,
        context,
        diagnostic_code,
        temporaries,
    )?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::BoolComparison {
            operator,
            left: Box::new(left.value),
            right: Box::new(right.value),
        },
    })
}

fn lower_bool_comparison_operand_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_normal_call(call, temporary, context, temporaries)?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::BoolLiteral(_) | Expr::Identifier(_) => Ok(LoweredBoolValue {
            instructions: Vec::new(),
            value: lower_bool_comparison_operand(expression, context, diagnostic_code)?,
        }),
        Expr::Group(group) => lower_bool_comparison_operand_to_value_with_temporaries(
            &group.expression,
            context,
            diagnostic_code,
            temporaries,
        ),
        _ => Err(unsupported_bool_comparison_operand_diagnostic(
            diagnostic_code,
        )),
    }
}

fn lower_i32_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_i32_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_i32_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::I32Comparison {
            operator,
            left: left.value,
            right: right.value,
        },
    })
}

fn lower_usize_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_usize_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_usize_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::UsizeComparison {
            operator,
            left: left.value,
            right: right.value,
        },
    })
}

fn lower_str_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .str_location(&identifier.name)
            .map(StrValue::Location)
            .ok_or_else(unsupported_str_expression_diagnostic),
        Expr::Group(group) => lower_str_value(&group.expression, context),
        _ => lower_str_literal(expression),
    }
}

fn lower_slice_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<SliceValue, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .slice_location(&identifier.name)
            .map(SliceValue::Location)
            .ok_or_else(unsupported_slice_expression_diagnostic),
        Expr::Group(group) => lower_slice_value(&group.expression, context),
        _ => Err(unsupported_slice_expression_diagnostic()),
    }
}

pub(super) fn lower_i32_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .i32_location(&identifier.name)
            .map(I32Value::Location)
            .ok_or_else(unsupported_i32_expression_diagnostic),
        Expr::Group(group) => lower_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).map(I32Value::Const),
    }
}

pub(super) fn lower_u8_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<U8Value, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .u8_location(&identifier.name)
            .map(U8Value::Location)
            .ok_or_else(unsupported_u8_expression_diagnostic),
        Expr::Group(group) => lower_u8_value(&group.expression, context),
        _ => lower_u8_literal(expression).map(U8Value::Const),
    }
}

pub(super) fn lower_usize_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::Identifier(identifier) => context
            .usize_location(&identifier.name)
            .map(UsizeValue::Location)
            .ok_or_else(unsupported_usize_expression_diagnostic),
        Expr::Group(group) => lower_usize_value(&group.expression, context),
        _ => lower_usize_literal(expression).map(UsizeValue::Const),
    }
}

fn lower_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    if let Ok(value) = lower_str_value(&expression.object, context) {
        return Ok(LoweredU8Value {
            instructions: index.instructions,
            value: match value {
                StrValue::StaticBytes(bytes) => U8Value::StaticStrIndex {
                    bytes,
                    index: index.value,
                },
                StrValue::Location(source) => U8Value::StrIndex {
                    source,
                    index: index.value,
                },
            },
        });
    }

    if let Ok(SliceValue::Location(source)) = lower_slice_value(&expression.object, context) {
        return Ok(LoweredU8Value {
            instructions: index.instructions,
            value: U8Value::SliceIndex {
                source,
                index: index.value,
            },
        });
    }

    Err(unsupported_u8_expression_diagnostic())
}

fn lower_builtin_len_call_value(
    call: &crate::ast::CallExpr,
    context: &LoweringContext,
) -> Option<Result<UsizeValue, Vec<Diagnostic>>> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "len" || !call.arguments.is_empty() {
        return None;
    }

    if let Ok(value) = lower_str_value(&member.object, context) {
        return Some(Ok(match value {
            StrValue::StaticBytes(bytes) => UsizeValue::Const(bytes.len() as u64),
            StrValue::Location(location) => UsizeValue::StrLen(location),
        }));
    }

    if let Ok(SliceValue::Location(location)) = lower_slice_value(&member.object, context) {
        return Some(Ok(UsizeValue::SliceLen(location)));
    }

    None
}

pub(super) fn lower_bool_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Unary(unary) => lower_bool_unary_value(unary, context, diagnostic_code),
        Expr::Binary(binary) => lower_bool_binary_value(binary, context, diagnostic_code),
        Expr::Group(group) => lower_bool_value(&group.expression, context, diagnostic_code),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn lower_bool_unary_value(
    unary: &UnaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match unary.operator {
        UnaryOperator::LogicalNot => Ok(BoolValue::Not(Box::new(lower_bool_value(
            &unary.operand,
            context,
            diagnostic_code,
        )?))),
        UnaryOperator::Negate => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn lower_bool_binary_value(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
            lower_bool_logical_value(binary, context, diagnostic_code)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if expressions_are_lowerable_bool_comparison_operands(
                &binary.left,
                &binary.right,
                context,
            ) =>
        {
            lower_bool_comparison_condition(binary, context, diagnostic_code)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual
            if expressions_are_lowerable_bool_values(&binary.left, &binary.right, context) =>
        {
            Err(unsupported_bool_comparison_operand_diagnostic(
                diagnostic_code,
            ))
        }
        _ if expressions_are_lowerable_usize_values(&binary.left, &binary.right, context) => {
            lower_usize_comparison_condition(binary, context, diagnostic_code)
        }
        _ => lower_i32_comparison_condition(binary, context, diagnostic_code),
    }
}

fn lower_bool_logical_value(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::LogicalAnd => BoolLogicalOperator::And,
        BinaryOperator::LogicalOr => BoolLogicalOperator::Or,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    Ok(BoolValue::Logical {
        operator,
        left: Box::new(lower_bool_value(&binary.left, context, diagnostic_code)?),
        right: Box::new(lower_bool_value(&binary.right, context, diagnostic_code)?),
    })
}

fn lower_bool_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::Equal => BoolComparisonOperator::Equal,
        BinaryOperator::NotEqual => BoolComparisonOperator::NotEqual,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    Ok(BoolValue::BoolComparison {
        operator,
        left: Box::new(lower_bool_comparison_operand(
            &binary.left,
            context,
            diagnostic_code,
        )?),
        right: Box::new(lower_bool_comparison_operand(
            &binary.right,
            context,
            diagnostic_code,
        )?),
    })
}

fn lower_bool_comparison_operand(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Group(group) => {
            lower_bool_comparison_operand(&group.expression, context, diagnostic_code)
        }
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn lower_i32_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;

    Ok(BoolValue::I32Comparison {
        operator,
        left: lower_i32_value(&binary.left, context)?,
        right: lower_i32_value(&binary.right, context)?,
    })
}

fn lower_usize_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = i32_comparison_operator(binary.operator, diagnostic_code)?;

    Ok(BoolValue::UsizeComparison {
        operator,
        left: lower_usize_value(&binary.left, context)?,
        right: lower_usize_value(&binary.right, context)?,
    })
}

fn i32_comparison_operator(
    operator: BinaryOperator,
    diagnostic_code: &'static str,
) -> Result<I32ComparisonOperator, Vec<Diagnostic>> {
    match operator {
        BinaryOperator::Equal => Ok(I32ComparisonOperator::Equal),
        BinaryOperator::NotEqual => Ok(I32ComparisonOperator::NotEqual),
        BinaryOperator::Less => Ok(I32ComparisonOperator::Less),
        BinaryOperator::LessEqual => Ok(I32ComparisonOperator::LessEqual),
        BinaryOperator::Greater => Ok(I32ComparisonOperator::Greater),
        BinaryOperator::GreaterEqual => Ok(I32ComparisonOperator::GreaterEqual),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}

fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, arithmetic or shift expressions, and direct tail calls",
    )]
}

fn unsupported_u8_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower u8 literals, parameters, locals, direct tail calls, and indexing into `&str`, `&[u8]`, or `&+[u8]`",
    )]
}

fn unsupported_usize_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower usize literals, parameters, arithmetic or shift expressions, and direct tail calls",
    )]
}

fn unsupported_str_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower string literals and `&str` parameters as `&str` values",
    )]
}

fn unsupported_slice_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower slice parameters and locals as slice values",
    )]
}

fn unsupported_non_tail_call_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower function calls in direct tail return position",
    )]
}

fn unsupported_bool_comparison_operand_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals",
    )]
}

fn unsupported_bool_expression_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool literals, bool locals, bool operators, i32 or usize comparisons, and bool equality/inequality over bool literals or bool locals",
    )]
}
