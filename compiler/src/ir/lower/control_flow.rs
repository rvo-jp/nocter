use super::context::LoweringContext;
use super::expressions::{
    expression_contains_call, lower_bool_expression_to_value, lower_bool_return_expression,
    lower_i32_return_expression, lower_slice_return_expression, lower_str_return_expression,
    lower_u8_return_expression, lower_usize_return_expression,
};
use super::functions::{
    append_scope_end_drops_before_exit, lower_value_return_with_scope_drops,
    mark_explicit_moves_in_expression,
};
use crate::ast::{BinaryExpr, BinaryOperator, Block, Expr, IfStmt, Stmt};
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, Type};

type ReturnLowerer = fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>;

pub(super) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "i32",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_i32_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
        )?,
        lower_i32_return_block(else_block, context, return_type, diagnostic_code, subject)?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_bool_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_bool_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
        )?,
        lower_bool_return_block(else_block, context, return_type, diagnostic_code, subject)?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_u8_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "u8",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_scalar_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "u8",
            lower_u8_return_expression,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "u8",
            lower_u8_return_expression,
        )?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_usize_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "usize",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_scalar_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "usize",
            lower_usize_return_expression,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "usize",
            lower_usize_return_expression,
        )?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_str_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "&str",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_scalar_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "&str",
            lower_str_return_expression,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            "&str",
            lower_str_return_expression,
        )?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_slice_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_label = match return_type.success_type() {
        Type::Slice { is_readwrite: true } => "&+[u8]",
        _ => "&[u8]",
    };
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_scalar_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_slice_return_expression,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_slice_return_expression,
        )?,
        context,
        diagnostic_code,
    )
}

fn lower_terminal_condition(
    condition: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(binary) = short_circuit_condition_with_call(condition) {
        return lower_short_circuit_terminal_condition(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
        );
    }

    let condition = lower_bool_expression_to_value(condition, context, diagnostic_code)?;
    let mut instructions = condition.instructions;
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions,
        else_instructions,
    });
    Ok(instructions)
}

fn lower_short_circuit_terminal_condition(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => lower_terminal_condition(
            &binary.left,
            lower_terminal_condition(
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
        BinaryOperator::LogicalOr => lower_terminal_condition(
            &binary.left,
            then_instructions.clone(),
            lower_terminal_condition(
                &binary.right,
                then_instructions,
                else_instructions,
                context,
                diagnostic_code,
            )?,
            context,
            diagnostic_code,
        ),
        _ => unreachable!("short-circuit condition must be && or ||"),
    }
}

fn short_circuit_condition_with_call(condition: &Expr) -> Option<&BinaryExpr> {
    let condition = unwrap_group(condition);
    let Expr::Binary(binary) = condition else {
        return None;
    };

    match binary.operator {
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
            if expression_contains_call(&binary.left)
                || expression_contains_call(&binary.right) =>
        {
            Some(binary)
        }
        _ => None,
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn lower_i32_return_block(
    block: &Block,
    context: &LoweringContext,
    return_type: &Type,
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
            let mut branch_context = context.clone();
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                return Ok(return_instructions);
            }
            let return_instructions = lower_i32_return_expression(expression, &branch_context)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            append_scope_end_drops_before_exit(return_instructions, &mut branch_context)
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
    return_type: &Type,
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
            let mut branch_context = context.clone();
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                return Ok(return_instructions);
            }
            let return_instructions =
                lower_bool_return_expression(expression, &branch_context, diagnostic_code)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            append_scope_end_drops_before_exit(return_instructions, &mut branch_context)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        )),
    }
}

fn lower_scalar_return_block(
    block: &Block,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match block.statements.as_slice() {
        [Stmt::Return(statement)] => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    return_label,
                ));
            };
            let mut branch_context = context.clone();
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                return Ok(return_instructions);
            }
            let return_instructions = lower_return_expression(expression, &branch_context)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            append_scope_end_drops_before_exit(return_instructions, &mut branch_context)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
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
