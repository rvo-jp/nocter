use super::bindings::{lower_assignment, lower_local_binding};
use super::context::LoweringContext;
use super::expressions::{
    expression_contains_call, lower_bool_expression_to_value, lower_bool_return_expression,
    lower_i32_return_expression, lower_slice_return_expression, lower_str_return_expression,
    lower_u8_return_expression, lower_usize_return_expression, lower_void_expression_statement,
};
use super::functions::{
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_drop_statement,
    lower_scope_end_drops_for_locals_since, lower_value_return_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
};
use crate::ast::{BinaryExpr, BinaryOperator, Block, Expr, IfStmt, Stmt, WhileStmt};
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
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "u8",
        lower_u8_return_expression,
    )
}

pub(super) fn lower_terminal_usize_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "usize",
        lower_usize_return_expression,
    )
}

pub(super) fn lower_terminal_str_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "&str",
        lower_str_return_expression,
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

    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_slice_return_expression,
    )
}

fn lower_terminal_scalar_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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
            lower_return_expression,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_return_expression,
        )?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_void_if_statement(
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
            "void",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_void_return_block(
            &statement.then_block,
            context,
            return_type,
            diagnostic_code,
            subject,
        )?,
        lower_void_return_block(else_block, context, return_type, diagnostic_code, subject)?,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_terminal_condition(
    condition: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(condition, context) {
        return Err(unsupported_control_flow_condition_move_diagnostic(
            diagnostic_code,
        ));
    }

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

pub(super) fn lower_nonterminal_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_nonterminal_if_block(
        &statement.then_block,
        context,
        loop_scope_mark,
        diagnostic_code,
        subject,
    )?;
    let else_instructions = if let Some(else_block) = &statement.else_block {
        lower_nonterminal_if_block(
            else_block,
            context,
            loop_scope_mark,
            diagnostic_code,
            subject,
        )?
    } else {
        Vec::new()
    };

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_nonterminal_while_statement(
    statement: &WhileStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(unsupported_control_flow_condition_move_diagnostic(
            diagnostic_code,
        ));
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, diagnostic_code)?;
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject)?;

    Ok(vec![Instruction::While {
        condition_instructions: condition.instructions,
        condition: condition.value,
        body_instructions,
    }])
}

fn lower_nonterminal_while_block(
    block: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let mut instructions = lower_nonterminal_loop_block_statements(
        &block.statements,
        &mut body_context,
        local_mark,
        Some(local_mark),
        diagnostic_code,
        subject,
    )?;
    instructions.extend(lower_scope_end_drops_for_locals_since(
        &mut body_context,
        local_mark,
    )?);
    Ok(instructions)
}

fn lower_nonterminal_if_block(
    block: &Block,
    context: &LoweringContext,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let mut instructions = lower_nonterminal_loop_block_statements(
        &block.statements,
        &mut branch_context,
        local_mark,
        loop_scope_mark,
        diagnostic_code,
        subject,
    )?;
    instructions.extend(lower_scope_end_drops_for_locals_since(
        &mut branch_context,
        local_mark,
    )?);
    Ok(instructions)
}

fn lower_nonterminal_loop_block_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    local_mark: usize,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                }
                instructions.extend(lower_local_binding(statement, context)?)
            }
            Stmt::Assignment(statement) => {
                let Some(target_name) = assignment_target_root_name(&statement.target) else {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                };
                if !context.local_defined_since(target_name, local_mark)
                    || expression_contains_explicit_aggregate_move_outside(
                        &statement.value,
                        context,
                        local_mark,
                    )
                {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                }
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                }
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                if !context.aggregate_local_defined_since(&statement.name, local_mark) {
                    return Err(unsupported_nonterminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                    ));
                }
                instructions.extend(lower_drop_statement(statement, context)?);
            }
            Stmt::If(statement) => instructions.extend(lower_nonterminal_if_statement(
                statement,
                context,
                loop_scope_mark,
                diagnostic_code,
                subject,
            )?),
            Stmt::While(statement) => instructions.extend(lower_nonterminal_while_statement(
                statement,
                context,
                diagnostic_code,
                subject,
            )?),
            Stmt::Break(_) => {
                instructions.extend(lower_nonterminal_loop_control_statement(
                    Instruction::Break,
                    context,
                    loop_scope_mark,
                    diagnostic_code,
                    subject,
                )?);
                break;
            }
            Stmt::Continue(_) => {
                instructions.extend(lower_nonterminal_loop_control_statement(
                    Instruction::Continue,
                    context,
                    loop_scope_mark,
                    diagnostic_code,
                    subject,
                )?);
                break;
            }
            _ => {
                return Err(unsupported_nonterminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                ));
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    Ok(instructions)
}

fn lower_nonterminal_loop_control_statement(
    instruction: Instruction,
    context: &mut LoweringContext,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(loop_scope_mark) = loop_scope_mark else {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    };

    let mut instructions = lower_scope_end_drops_for_locals_since(context, loop_scope_mark)?;
    instructions.push(instruction);
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

fn assignment_target_root_name(expression: &Expr) -> Option<&str> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(&identifier.name),
        Expr::Member(member) => assignment_target_root_name(&member.object),
        _ => None,
    }
}

fn lower_i32_return_block(
    block: &Block,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "i32")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "i32",
    )?;

    match terminal {
        Stmt::Return(statement) => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "i32",
                ));
            };
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }
            let return_instructions = lower_i32_return_expression(expression, &branch_context)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            instructions.extend(return_instructions);
            append_scope_end_drops_before_exit(instructions, &mut branch_context)
        }
        Stmt::If(statement) => {
            instructions.extend(lower_terminal_i32_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
            )?);
            Ok(instructions)
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
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "bool")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "bool",
    )?;

    match terminal {
        Stmt::Return(statement) => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "bool",
                ));
            };
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }
            let return_instructions =
                lower_bool_return_expression(expression, &branch_context, diagnostic_code)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            instructions.extend(return_instructions);
            append_scope_end_drops_before_exit(instructions, &mut branch_context)
        }
        Stmt::If(statement) => {
            instructions.extend(lower_terminal_bool_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
            )?);
            Ok(instructions)
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
    let (terminal, leading) =
        split_terminal_branch_block(block, diagnostic_code, subject, return_label)?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        return_label,
    )?;

    match terminal {
        Stmt::Return(statement) => {
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    return_label,
                ));
            };
            if let Some(return_instructions) = lower_value_return_with_scope_drops(
                return_type.success_type(),
                expression,
                return_type,
                &mut branch_context,
            )? {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }
            let return_instructions = lower_return_expression(expression, &branch_context)?;
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            instructions.extend(return_instructions);
            append_scope_end_drops_before_exit(instructions, &mut branch_context)
        }
        Stmt::If(statement) => {
            instructions.extend(lower_terminal_scalar_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        )),
    }
}

fn lower_void_return_block(
    block: &Block,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "void")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "void",
    )?;

    match terminal {
        Stmt::Return(statement) if statement.expression.is_none() => {
            instructions.push(Instruction::Return);
            append_scope_end_drops_before_exit(instructions, &mut branch_context)
        }
        Stmt::If(statement) => {
            instructions.extend(lower_terminal_void_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "void",
        )),
    }
}

pub(super) fn split_terminal_branch_block<'a>(
    block: &'a Block,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
) -> Result<(&'a Stmt, &'a [Stmt]), Vec<Diagnostic>> {
    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        ));
    };
    Ok((terminal, leading))
}

pub(super) fn lower_terminal_branch_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?)
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?)
            }
            Stmt::Drop(statement) => instructions.extend(lower_drop_statement(statement, context)?),
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_terminal_if_diagnostic(
                        diagnostic_code,
                        subject,
                        return_label,
                    ));
                };
                instructions.extend(void_instructions);
            }
            _ => {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    return_label,
                ));
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    Ok(instructions)
}

fn unsupported_terminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
    return_type: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower terminal `if` statements for {subject} when both branches contain only supported binding, assignment, explicit `drop`, or void call statements followed by returns or nested terminal `if` branches returning `{return_type}`"
        ),
    )]
}

fn unsupported_nonterminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower non-terminal `if`/`while` statements for {subject} when branches contain supported local bindings, branch/body-local assignments or explicit aggregate drops, void call statements, or nested non-terminal `if`/`while` statements"
        ),
    )]
}

fn unsupported_control_flow_condition_move_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 cannot lower control-flow conditions that explicitly move aggregate values",
    )]
}
