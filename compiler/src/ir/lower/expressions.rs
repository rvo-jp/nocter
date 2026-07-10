use super::context::LoweringContext;
use super::literals::lower_i32_literal;
use crate::ast::{
    BinaryExpr, BinaryOperator, CallExpr, Expr, InterpolatedStringPart, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue, I32ComparisonOperator,
    I32Location, I32Value, Instruction, Type,
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
        Expr::Binary(binary) if is_i32_arithmetic_operator(binary.operator) => {
            lower_i32_arithmetic_expression_to_location(binary, destination, context)
        }
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

fn lower_i32_arithmetic_expression_to_location(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_i32_arithmetic_expression_to_location_with_temporaries(
        binary,
        destination,
        context,
        &mut temporaries,
    )
}

fn lower_i32_arithmetic_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let left = lower_i32_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_i32_expression_to_value(&binary.right, context, temporaries)?;
    let mut instructions = left.instructions;
    instructions.extend(right.instructions);
    instructions.push(i32_arithmetic_instruction(
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
        Expr::Binary(binary) if is_i32_arithmetic_operator(binary.operator) => {
            let temporary = temporaries.next_i32()?;
            Ok(LoweredI32Value {
                instructions: lower_i32_arithmetic_expression_to_location_with_temporaries(
                    binary,
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

fn i32_arithmetic_instruction(
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
        _ => Err(unsupported_i32_expression_diagnostic()),
    }
}

struct LoweredI32Value {
    instructions: Vec<Instruction>,
    value: I32Value,
}

struct TemporaryAllocator {
    next_index: usize,
}

impl TemporaryAllocator {
    fn new(context: &LoweringContext) -> Result<Self, Vec<Diagnostic>> {
        Ok(Self {
            next_index: context.first_temporary_local_index()?,
        })
    }

    fn next_i32(&mut self) -> Result<I32Location, Vec<Diagnostic>> {
        if self.next_index >= MAX_TEMPORARY_SCALAR_LOCALS {
            return Err(vec![Diagnostic::error(
                "E8008",
                format!(
                    "IR v0 can only lower up to {MAX_TEMPORARY_SCALAR_LOCALS} local scalar bindings"
                ),
            )]);
        }

        let location = I32Location::Local(self.next_index);
        self.next_index += 1;
        Ok(location)
    }

    fn next_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>> {
        if self.next_index >= MAX_TEMPORARY_SCALAR_LOCALS {
            return Err(vec![Diagnostic::error(
                "E8008",
                format!(
                    "IR v0 can only lower up to {MAX_TEMPORARY_SCALAR_LOCALS} local scalar bindings"
                ),
            )]);
        }

        let location = BoolLocation::Local(self.next_index);
        self.next_index += 1;
        Ok(location)
    }
}

const MAX_TEMPORARY_SCALAR_LOCALS: usize = 7;

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
        Expr::Binary(binary) if i32_comparison_contains_call(binary, context) => {
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

fn short_circuit_bool_expression_contains_call(binary: &BinaryExpr) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
    ) && (expression_contains_call(&binary.left) || expression_contains_call(&binary.right))
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn expression_contains_call(expression: &Expr) -> bool {
    match expression {
        Expr::Call(_) => true,
        Expr::Unary(unary) => expression_contains_call(&unary.operand),
        Expr::Binary(binary) => {
            expression_contains_call(&binary.left) || expression_contains_call(&binary.right)
        }
        Expr::Group(group) => expression_contains_call(&group.expression),
        Expr::TypeConversion(conversion) => expression_contains_call(&conversion.expression),
        Expr::Propagate(propagation) => expression_contains_call(&propagation.expression),
        Expr::Force(force) => expression_contains_call(&force.expression),
        Expr::Catch(catch) => expression_contains_call(&catch.expression),
        Expr::Member(member) => expression_contains_call(&member.object),
        Expr::Index(index) => {
            expression_contains_call(&index.object) || expression_contains_call(&index.index)
        }
        Expr::ArrayLiteral(array) => array.elements.iter().any(expression_contains_call),
        Expr::StructLiteral(struct_literal) => struct_literal
            .fields
            .iter()
            .any(|field| expression_contains_call(&field.value)),
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().any(|part| {
            matches!(
                part,
                InterpolatedStringPart::Expression(part)
                    if expression_contains_call(&part.expression)
            )
        }),
        Expr::OptionalDefault(optional_default) => {
            expression_contains_call(&optional_default.value)
                || expression_contains_call(&optional_default.default)
        }
        Expr::PatternConditional(pattern_conditional) => {
            expression_contains_call(&pattern_conditional.target)
                || pattern_conditional
                    .arms
                    .iter()
                    .any(|arm| expression_contains_call(&arm.expression))
                || expression_contains_call(&pattern_conditional.fallback)
        }
        _ => false,
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
        Expr::Binary(binary) if i32_comparison_contains_call(binary, context) => {
            lower_i32_comparison_to_value_with_temporaries(
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

fn bool_comparison_contains_call(binary: &BinaryExpr, context: &LoweringContext) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_comparison_operands_with_calls(
        &binary.left,
        &binary.right,
        context,
    )
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

fn i32_comparison_contains_call(binary: &BinaryExpr, context: &LoweringContext) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_i32_values_with_calls(&binary.left, &binary.right, context)
}

fn lower_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_normal_call_return_type(&identifier.name, context)?;

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for argument in &call.arguments {
        let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
        instructions.extend(argument.instructions);
        arguments.push(argument.value);
    }

    instructions.push(Instruction::CallI32 {
        destination,
        function: identifier.name.clone(),
        arguments,
    });
    Ok(instructions)
}

fn lower_bool_normal_call(
    call: &CallExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_bool_normal_call_return_type(&identifier.name, context)?;

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for argument in &call.arguments {
        let argument = lower_i32_expression_to_value(argument, context, temporaries)?;
        instructions.extend(argument.instructions);
        arguments.push(argument.value);
    }

    instructions.push(Instruction::CallBool {
        destination,
        function: identifier.name.clone(),
        arguments,
    });
    Ok(instructions)
}

fn lower_direct_tail_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls in tail return position",
        )]);
    };

    validate_tail_call_return_type(&identifier.name, context)?;

    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    for argument in &call.arguments {
        let argument = lower_i32_expression_to_value(argument, context, &mut temporaries)?;
        instructions.extend(argument.instructions);
        arguments.push(argument.value);
    }

    instructions.push(Instruction::TailCall {
        function: identifier.name.clone(),
        arguments,
    });
    Ok(instructions)
}

fn validate_normal_call_return_type(
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.function_return_type(callee_name) else {
        return Ok(());
    };

    if callee_return_type == &Type::I32 {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `i32`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_bool_normal_call_return_type(
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.function_return_type(callee_name) else {
        return Ok(());
    };

    if callee_return_type == &Type::Bool {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `bool`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_tail_call_return_type(
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.function_return_type(callee_name) else {
        return Ok(());
    };

    if callee_return_type == context.return_type() {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower tail call from function `{}` returning `{}` to function `{callee_name}` returning `{}`",
            context.function_name(),
            describe_type(context.return_type()),
            describe_type(callee_return_type),
        ),
    )])
}

fn describe_type(ty: &Type) -> &'static str {
    match ty {
        Type::I32 => "i32",
        Type::Bool => "bool",
        Type::Void => "void",
        Type::Fallible(success) => match success.as_ref() {
            Type::I32 => "i32!",
            Type::Bool => "bool!",
            Type::Void => "void!",
            Type::Fallible(_) => "fallible",
        },
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

pub(super) fn expression_is_lowerable_bool_binding(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => context.bool_location(&identifier.name).is_some(),
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_lowerable_bool_binding(&unary.operand, context)
        }
        Expr::Binary(binary) => {
            expression_is_lowerable_comparison_binding(binary, context)
                || (is_bool_logical_operator(binary.operator)
                    && expression_is_lowerable_bool_binding(&binary.left, context)
                    && expression_is_lowerable_bool_binding(&binary.right, context))
        }
        Expr::Group(group) => expression_is_lowerable_bool_binding(&group.expression, context),
        _ => false,
    }
}

pub(super) fn expression_is_unsupported_bool_comparison_binding(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Binary(binary) => {
            is_bool_equality_operator(binary.operator)
                && expressions_are_lowerable_bool_values(&binary.left, &binary.right, context)
                && !expressions_are_lowerable_bool_comparison_operands(
                    &binary.left,
                    &binary.right,
                    context,
                )
        }
        Expr::Group(group) => {
            expression_is_unsupported_bool_comparison_binding(&group.expression, context)
        }
        _ => false,
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

fn is_i32_comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual
    )
}

fn is_i32_arithmetic_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Remainder
    )
}

fn expression_is_lowerable_comparison_binding(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    if is_i32_comparison_operator(binary.operator)
        && (expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
            || expressions_are_lowerable_i32_values_with_calls(
                &binary.left,
                &binary.right,
                context,
            ))
    {
        return true;
    }

    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && (expressions_are_lowerable_bool_comparison_operands(&binary.left, &binary.right, context)
        || expressions_are_lowerable_bool_comparison_operands_with_calls(
            &binary.left,
            &binary.right,
            context,
        ))
}

fn expressions_are_lowerable_i32_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_i32_value(left, context)
        && expression_is_lowerable_i32_value(right, context)
}

fn expressions_are_lowerable_i32_values_with_calls(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_i32_expression_with_calls(left, context)
        && expression_is_lowerable_i32_expression_with_calls(right, context)
        && (expression_contains_call(left) || expression_contains_call(right))
}

fn expression_is_lowerable_i32_expression_with_calls(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.function_return_type(&identifier.name) == Some(&Type::I32)
        }
        Expr::Binary(binary) if is_i32_arithmetic_operator(binary.operator) => {
            expression_is_lowerable_i32_expression_with_calls(&binary.left, context)
                && expression_is_lowerable_i32_expression_with_calls(&binary.right, context)
        }
        Expr::Group(group) => {
            expression_is_lowerable_i32_expression_with_calls(&group.expression, context)
        }
        _ => expression_is_lowerable_i32_value(expression, context),
    }
}

fn expression_is_lowerable_i32_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.i32_location(&identifier.name).is_some(),
        Expr::Group(group) => expression_is_lowerable_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).is_ok(),
    }
}

fn expressions_are_lowerable_bool_comparison_operands(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand(left, context)
        && expression_is_lowerable_bool_comparison_operand(right, context)
}

fn expressions_are_lowerable_bool_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_binding(left, context)
        && expression_is_lowerable_bool_binding(right, context)
}

fn expression_is_lowerable_bool_comparison_operand(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => context.bool_location(&identifier.name).is_some(),
        Expr::Group(group) => {
            expression_is_lowerable_bool_comparison_operand(&group.expression, context)
        }
        _ => false,
    }
}

fn expressions_are_lowerable_bool_comparison_operands_with_calls(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand_or_call(left, context)
        && expression_is_lowerable_bool_comparison_operand_or_call(right, context)
        && (expression_contains_call(left) || expression_contains_call(right))
}

fn expression_is_lowerable_bool_comparison_operand_or_call(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand(expression, context)
        || expression_is_direct_bool_returning_call(expression, context)
}

fn expression_is_direct_bool_returning_call(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.function_return_type(&identifier.name) == Some(&Type::Bool)
        }
        Expr::Group(group) => expression_is_direct_bool_returning_call(&group.expression, context),
        _ => false,
    }
}

fn is_bool_logical_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
    )
}

fn is_bool_equality_operator(operator: BinaryOperator) -> bool {
    matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
}

fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, arithmetic, and direct tail calls",
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
        "IR v0 can only lower bool literals, bool locals, bool operators, i32 comparisons, and bool equality/inequality over bool literals or bool locals",
    )]
}
