use super::context::LoweringContext;
use super::literals::lower_i32_literal;
use crate::ast::{BinaryExpr, BinaryOperator, CallExpr, Expr, UnaryExpr, UnaryOperator};
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
        Expr::Call(call) => lower_i32_normal_call(call, destination, context),
        Expr::Binary(binary) if binary.operator == BinaryOperator::Add => {
            lower_i32_add_expression_to_location(binary, destination, context)
        }
        Expr::Group(group) => {
            lower_i32_expression_to_location(&group.expression, destination, context)
        }
        _ => lower_i32_value(expression, context)
            .map(|value| vec![Instruction::SetI32 { destination, value }]),
    }
}

fn lower_i32_add_expression_to_location(
    binary: &BinaryExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match (binary.left.as_ref(), binary.right.as_ref()) {
        (Expr::Call(_), Expr::Call(_)) => Err(unsupported_non_tail_call_diagnostic()),
        (Expr::Call(call), right) => {
            let temporary = context.next_i32_temporary_location()?;
            let mut instructions = lower_i32_normal_call(call, temporary, context)?;
            instructions.push(Instruction::AddI32 {
                destination,
                left: I32Value::Location(temporary),
                right: lower_i32_value(right, context)?,
            });
            Ok(instructions)
        }
        (left, Expr::Call(call)) => {
            let temporary = context.next_i32_temporary_location()?;
            let mut instructions = lower_i32_normal_call(call, temporary, context)?;
            instructions.push(Instruction::AddI32 {
                destination,
                left: lower_i32_value(left, context)?,
                right: I32Value::Location(temporary),
            });
            Ok(instructions)
        }
        (left, right) => Ok(vec![Instruction::AddI32 {
            destination,
            left: lower_i32_value(left, context)?,
            right: lower_i32_value(right, context)?,
        }]),
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
        _ => Ok(vec![
            Instruction::SetBool {
                destination: BoolLocation::Return,
                value: lower_bool_value(expression, context, diagnostic_code)?,
            },
            Instruction::Return,
        ]),
    }
}

fn lower_i32_normal_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_non_tail_call_diagnostic());
    };

    validate_normal_call_return_type(&identifier.name, context)?;

    let mut arguments = Vec::new();
    for argument in &call.arguments {
        arguments.push(lower_i32_value(argument, context)?);
    }

    Ok(vec![Instruction::CallI32 {
        destination,
        function: identifier.name.clone(),
        arguments,
    }])
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

    let mut arguments = Vec::new();
    for (index, argument) in call.arguments.iter().enumerate() {
        arguments.push(lower_i32_call_argument(argument, index, context)?);
    }

    Ok(vec![Instruction::TailCall {
        function: identifier.name.clone(),
        arguments,
    }])
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

fn expression_is_lowerable_comparison_binding(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    if is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
    {
        return true;
    }

    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_comparison_operands(&binary.left, &binary.right, context)
}

fn expressions_are_lowerable_i32_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_i32_value(left, context)
        && expression_is_lowerable_i32_value(right, context)
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
        "IR v0 can only lower i32 literals, parameters, addition, and direct tail calls",
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
