use super::expressions::{I32ExpressionContext, lower_i32_return_expression};
use crate::ast::{Block, Expr, IfStmt, Stmt};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};

pub(super) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &I32ExpressionContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let condition = lower_bool_condition(&statement.condition, diagnostic_code)?;
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(diagnostic_code, subject));
    };

    Ok(vec![Instruction::If {
        condition,
        then_instructions: lower_i32_return_block(
            &statement.then_block,
            context,
            diagnostic_code,
            subject,
        )?,
        else_instructions: lower_i32_return_block(else_block, context, diagnostic_code, subject)?,
    }])
}

fn lower_bool_condition(
    expression: &Expr,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(vec![Diagnostic::error(
                diagnostic_code,
                "IR v0 can only lower bool literal `if` conditions",
            )]),
        },
        Expr::Group(group) => lower_bool_condition(&group.expression, diagnostic_code),
        _ => Err(vec![Diagnostic::error(
            diagnostic_code,
            "IR v0 can only lower bool literal `if` conditions",
        )]),
    }
}

fn lower_i32_return_block(
    block: &Block,
    context: &I32ExpressionContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match block.statements.as_slice() {
        [Stmt::Return(statement)] => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(diagnostic_code, subject));
            };
            lower_i32_return_expression(expression, context)
        }
        _ => Err(unsupported_terminal_if_diagnostic(diagnostic_code, subject)),
    }
}

fn unsupported_terminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower terminal `if` statements for {subject} when both branches directly return `i32`"
        ),
    )]
}
