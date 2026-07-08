use super::context::LoweringContext;
use super::expressions::{
    lower_bool_return_expression, lower_bool_value, lower_i32_return_expression,
};
use crate::ast::{Block, IfStmt, Stmt};
use crate::diagnostics::Diagnostic;
use crate::ir::Instruction;

pub(super) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let condition = lower_bool_value(&statement.condition, context, diagnostic_code)?;
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "i32",
        ));
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

pub(super) fn lower_terminal_bool_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let condition = lower_bool_value(&statement.condition, context, diagnostic_code)?;
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        ));
    };

    Ok(vec![Instruction::If {
        condition,
        then_instructions: lower_bool_return_block(
            &statement.then_block,
            context,
            diagnostic_code,
            subject,
        )?,
        else_instructions: lower_bool_return_block(else_block, context, diagnostic_code, subject)?,
    }])
}

fn lower_i32_return_block(
    block: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match block.statements.as_slice() {
        [Stmt::Return(statement)] => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "i32",
                ));
            };
            lower_i32_return_expression(expression, context)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "i32",
        )),
    }
}

fn lower_bool_return_block(
    block: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match block.statements.as_slice() {
        [Stmt::Return(statement)] => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "bool",
                ));
            };
            lower_bool_return_expression(expression, context, diagnostic_code)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        )),
    }
}

fn unsupported_terminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
    return_type: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower terminal `if` statements for {subject} when both branches directly return `{return_type}`"
        ),
    )]
}
