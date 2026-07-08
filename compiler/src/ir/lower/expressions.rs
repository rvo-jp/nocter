use super::context::LoweringContext;
use super::literals::lower_i32_literal;
use crate::ast::{BinaryExpr, BinaryOperator, CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, I32ComparisonOperator, I32Location, I32Value, Instruction};

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
        Expr::Call(_) => Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower function calls in tail return position",
        )]),
        Expr::Binary(binary) if binary.operator == BinaryOperator::Add => {
            Ok(vec![Instruction::AddI32 {
                destination,
                left: lower_i32_value(&binary.left, context)?,
                right: lower_i32_value(&binary.right, context)?,
            }])
        }
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

pub(super) fn lower_i32_return_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_i32_tail_call(call, context),
        Expr::Group(group) => lower_i32_return_expression(&group.expression, context),
        _ => {
            let mut instructions = lower_i32_expression(expression, context)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

fn lower_i32_tail_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls",
        )]);
    };

    let mut arguments = Vec::new();
    for (index, argument) in call.arguments.iter().enumerate() {
        arguments.push(lower_i32_call_argument(argument, index, context)?);
    }

    Ok(vec![Instruction::TailCall {
        function: identifier.name.clone(),
        arguments,
    }])
}

fn lower_i32_call_argument(
    expression: &Expr,
    index: usize,
    context: &LoweringContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    let value = lower_i32_value(expression, context)?;

    if matches!(
        value,
        I32Value::Location(I32Location::Parameter(parameter)) if parameter != index
    ) {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 cannot lower reordered parameter call arguments",
        )]);
    }

    Ok(value)
}

pub(super) fn lower_i32_value(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    match expression {
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
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Binary(binary) => lower_i32_comparison_condition(binary, context, diagnostic_code),
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
        Expr::Binary(binary) => is_i32_comparison_operator(binary.operator),
        Expr::Group(group) => expression_is_lowerable_bool_binding(&group.expression, context),
        _ => false,
    }
}

fn lower_i32_comparison_condition(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    let operator = match binary.operator {
        BinaryOperator::Equal => I32ComparisonOperator::Equal,
        BinaryOperator::NotEqual => I32ComparisonOperator::NotEqual,
        BinaryOperator::Less => I32ComparisonOperator::Less,
        BinaryOperator::LessEqual => I32ComparisonOperator::LessEqual,
        BinaryOperator::Greater => I32ComparisonOperator::Greater,
        BinaryOperator::GreaterEqual => I32ComparisonOperator::GreaterEqual,
        _ => return Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    };

    Ok(BoolValue::I32Comparison {
        operator,
        left: lower_i32_value(&binary.left, context)?,
        right: lower_i32_value(&binary.right, context)?,
    })
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

fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, addition, and direct tail calls",
    )]
}

fn unsupported_bool_expression_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower bool literals, bool locals, and i32 comparisons",
    )]
}
