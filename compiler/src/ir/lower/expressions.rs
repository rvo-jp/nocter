use super::literals::lower_i32_literal;
use crate::ast::{BinaryOperator, CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::{I32Location, I32Value, Instruction};

pub(super) struct I32ExpressionContext {
    parameters: Vec<String>,
}

impl I32ExpressionContext {
    pub(super) fn empty() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    pub(super) fn new(parameters: Vec<String>) -> Self {
        Self { parameters }
    }

    fn parameter_location(&self, name: &str) -> Option<I32Location> {
        self.parameters
            .iter()
            .position(|parameter| parameter == name)
            .map(I32Location::Parameter)
    }
}

pub(super) fn lower_i32_expression(
    expression: &Expr,
    context: &I32ExpressionContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower function calls in tail return position",
        )]),
        Expr::Binary(binary) if binary.operator == BinaryOperator::Add => {
            Ok(vec![Instruction::AddI32 {
                destination: I32Location::Return,
                left: lower_i32_value(&binary.left, context)?,
                right: lower_i32_value(&binary.right, context)?,
            }])
        }
        Expr::Group(group) => lower_i32_expression(&group.expression, context),
        _ => lower_i32_value(expression, context).map(|value| {
            vec![Instruction::SetI32 {
                destination: I32Location::Return,
                value,
            }]
        }),
    }
}

pub(super) fn lower_i32_return_expression(
    expression: &Expr,
    context: &I32ExpressionContext,
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
    context: &I32ExpressionContext,
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
    context: &I32ExpressionContext,
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

fn lower_i32_value(
    expression: &Expr,
    context: &I32ExpressionContext,
) -> Result<I32Value, Vec<Diagnostic>> {
    match expression {
        Expr::Identifier(identifier) => context
            .parameter_location(&identifier.name)
            .map(I32Value::Location)
            .ok_or_else(unsupported_i32_expression_diagnostic),
        Expr::Group(group) => lower_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).map(I32Value::Const),
    }
}

fn unsupported_i32_expression_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        "IR v0 can only lower i32 literals, parameters, addition, and direct tail calls",
    )]
}
