use super::bindings::{
    assignment_targets_readwrite_aggregate_field, lower_assignment, lower_local_binding,
};
use super::context::LoweringContext;
use super::expressions::{
    expression_contains_call, lower_bool_expression_to_value, lower_bool_return_expression,
    lower_i32_return_expression, lower_slice_return_expression, lower_str_return_expression,
    lower_u8_return_expression, lower_usize_return_expression, lower_void_expression_statement,
    primitive_trap_call,
};
use super::functions::{
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_drop_statement,
    lower_never_expression_with_scope_drops, lower_return_statement_with_scope_drops,
    lower_scope_end_drops_for_locals_since, lower_value_return_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
};
use crate::ast::{BinaryExpr, BinaryOperator, Block, Expr, IfStmt, Stmt, WhileStmt};
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, Type};
use crate::source::{ByteSpan, SourceMap};

type ReturnLowerer = fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>;

struct LoweredNonterminalBlock {
    instructions: Vec<Instruction>,
    ends_execution: bool,
}

pub(super) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
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
            sources,
        )?,
        lower_i32_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_bool_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
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
            sources,
        )?,
        lower_bool_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_u8_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "u8",
        lower_u8_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_usize_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "usize",
        lower_usize_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_str_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "&str",
        lower_str_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_slice_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
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
        sources,
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
    sources: &SourceMap,
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
            sources,
        )?,
        lower_scalar_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_return_expression,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_void_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
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
            sources,
        )?,
        lower_void_return_block(
            else_block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_condition(
    condition: &Expr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            condition.span(),
        ));
    }

    if let Some(binary) = short_circuit_condition_with_call(condition) {
        return lower_short_circuit_terminal_condition(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
            sources,
        );
    }

    let condition = lower_bool_expression_to_value(condition, context, diagnostic_code).map_err(
        |diagnostics| attach_primary_span_if_absent(diagnostics, sources, condition.span()),
    )?;
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
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_nonterminal_if_block(
        &statement.then_block,
        context,
        loop_scope_mark,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = if let Some(else_block) = &statement.else_block {
        lower_nonterminal_if_block(
            else_block,
            context,
            loop_scope_mark,
            diagnostic_code,
            subject,
            sources,
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
        sources,
    )
}

pub(super) fn lower_nonterminal_while_statement(
    statement: &WhileStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            statement.condition.span(),
        ));
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, diagnostic_code)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.condition.span())
        })?;
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject, sources)?;

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
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        &mut body_context,
        local_mark,
        Some(local_mark),
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

fn lower_nonterminal_if_block(
    block: &Block,
    context: &LoweringContext,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        &mut branch_context,
        local_mark,
        loop_scope_mark,
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut branch_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

fn lower_nonterminal_loop_block_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    local_mark: usize,
    loop_scope_mark: Option<usize>,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<LoweredNonterminalBlock, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut ends_execution = false;
    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) && !outer_aggregate_move_binding_before_function_exit_allowed(
                    statement, context, local_mark, statements, index,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.initializer.span(),
                    ));
                }
                instructions.extend(lower_local_binding(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?)
            }
            Stmt::Assignment(statement) => {
                let target_allowed =
                    nonterminal_assignment_target_allowed(statement, context, local_mark)
                        || outer_aggregate_assignment_before_function_exit_allowed(
                            statement, context, local_mark, statements, index,
                        );
                let explicit_outer_aggregate_move_allowed =
                    aggregate_move_assignment_before_function_exit_allowed(
                        statement, context, local_mark, statements, index,
                    );
                if !target_allowed
                    || (expression_contains_explicit_aggregate_move_outside(
                        &statement.value,
                        context,
                        local_mark,
                    ) && !explicit_outer_aggregate_move_allowed)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.expression.span(),
                    ));
                }
                if let Some(terminating_instructions) =
                    lower_never_expression_with_scope_drops(&statement.expression, context)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(
                                diagnostics,
                                sources,
                                statement.expression.span(),
                            )
                        })?
                {
                    instructions.extend(terminating_instructions);
                    ends_execution = true;
                } else {
                    let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                    else {
                        return Err(attach_primary_span_if_absent(
                            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                            sources,
                            statement.span,
                        ));
                    };
                    instructions.extend(void_instructions);
                }
            }
            Stmt::Drop(statement) => {
                if !context.aggregate_local_defined_since(&statement.name, local_mark)
                    && !statement_suffix_exits_function(statements, index, context)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Return(statement) => {
                instructions.extend(
                    lower_return_statement_with_scope_drops(statement, context, diagnostic_code)
                        .map_err(|diagnostics| {
                            let span = statement
                                .expression
                                .as_ref()
                                .map_or(statement.span, |expression| expression.span());
                            attach_primary_span_if_absent(diagnostics, sources, span)
                        })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::If(statement) => {
                let lowered = lower_nonterminal_if_statement(
                    statement,
                    context,
                    loop_scope_mark,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::While(statement) => instructions.extend(
                lower_nonterminal_while_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Break(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Break,
                        context,
                        loop_scope_mark,
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::Continue(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Continue,
                        context,
                        loop_scope_mark,
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                    sources,
                    statement.span(),
                ));
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
        if ends_execution {
            break;
        }
    }
    Ok(LoweredNonterminalBlock {
        instructions,
        ends_execution,
    })
}

fn statement_suffix_exits_function(
    statements: &[Stmt],
    index: usize,
    context: &LoweringContext,
) -> bool {
    statement_sequence_exits_function(statements.get(index + 1..).unwrap_or(&[]), context)
}

fn statement_sequence_exits_function(statements: &[Stmt], context: &LoweringContext) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop(statement) {
            return false;
        }
        if statement_exits_function(statement, context) {
            return true;
        }
    }
    false
}

fn statement_exits_function(statement: &Stmt, context: &LoweringContext) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function(&statement.expression, context),
        Stmt::If(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        _ => false,
    }
}

fn block_exits_function(block: &Block, context: &LoweringContext) -> bool {
    statement_sequence_exits_function(&block.statements, context)
}

fn expression_exits_function(expression: &Expr, context: &LoweringContext) -> bool {
    let Expr::Call(call) = unwrap_group(expression) else {
        return false;
    };
    if primitive_trap_call(call, context) {
        return true;
    }
    let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
        return false;
    };
    context.call_return_type(&target) == Some(&Type::Never)
}

fn statement_may_exit_current_loop(statement: &Stmt) -> bool {
    match statement {
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Stmt::While(_) => false,
        _ => false,
    }
}

fn block_may_exit_current_loop(block: &Block) -> bool {
    block.statements.iter().any(statement_may_exit_current_loop)
}

fn outer_aggregate_move_binding_before_function_exit_allowed(
    statement: &crate::ast::BindingStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
) -> bool {
    statement_suffix_exits_function(statements, index, context)
        && direct_outer_aggregate_move(&statement.initializer, context, local_mark)
}

fn direct_outer_aggregate_move(
    expression: &Expr,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    let Expr::Unary(unary) = unwrap_group(expression) else {
        return false;
    };
    if unary.operator != crate::ast::UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return false;
    };
    context.aggregate_local(&identifier.name).is_some()
        && !context.aggregate_local_defined_since(&identifier.name, local_mark)
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

fn attach_primary_span_if_absent(
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
    span: ByteSpan,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.with_primary_span_if_absent(sources, span))
        .collect()
}

fn lower_short_circuit_terminal_condition(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    sources: &SourceMap,
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
                sources,
            )?,
            else_instructions,
            context,
            diagnostic_code,
            sources,
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
                sources,
            )?,
            context,
            diagnostic_code,
            sources,
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

fn nonterminal_assignment_target_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    assignment_target_root_name(&statement.target)
        .is_some_and(|target_name| context.local_defined_since(target_name, local_mark))
        || assignment_targets_readwrite_aggregate_field(statement, context)
}

fn outer_aggregate_assignment_before_function_exit_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
) -> bool {
    if !statement_suffix_exits_function(statements, index, context) {
        return false;
    }
    let Some(target_name) = assignment_target_root_name(&statement.target) else {
        return false;
    };
    context.aggregate_local(target_name).is_some()
        && !context.aggregate_local_defined_since(target_name, local_mark)
}

fn aggregate_move_assignment_before_function_exit_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
) -> bool {
    if !statement_suffix_exits_function(statements, index, context) {
        return false;
    }
    let Some(target_name) = assignment_target_root_name(&statement.target) else {
        return false;
    };
    context.aggregate_local(target_name).is_some()
        && direct_outer_aggregate_move(&statement.value, context, local_mark)
}

fn instruction_list_ends_execution(instructions: &[Instruction]) -> bool {
    match instructions.last() {
        Some(
            Instruction::Return
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::TailCall { .. }
            | Instruction::Trap
            | Instruction::Break
            | Instruction::Continue,
        ) => true,
        Some(Instruction::If {
            then_instructions,
            else_instructions,
            ..
        }) => {
            !else_instructions.is_empty()
                && instruction_list_ends_execution(then_instructions)
                && instruction_list_ends_execution(else_instructions)
        }
        _ => false,
    }
}

fn lower_i32_return_block(
    block: &Block,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "i32")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "i32",
        sources,
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
                sources,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "i32",
                ));
            };
            instructions.extend(terminating_instructions);
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
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "bool")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "bool",
        sources,
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
                sources,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "bool",
                ));
            };
            instructions.extend(terminating_instructions);
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
    sources: &SourceMap,
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
        sources,
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
                sources,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    return_label,
                ));
            };
            instructions.extend(terminating_instructions);
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
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "void")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "void",
        sources,
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
                sources,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "void",
                ));
            };
            instructions.extend(terminating_instructions);
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
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Binding(statement) => instructions.extend(
                lower_local_binding(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Assignment(statement) => instructions.extend(
                lower_assignment(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Drop(statement) => instructions.extend(
                lower_drop_statement(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Expression(statement) => {
                let Some(void_instructions) = lower_void_expression_statement(
                    &statement.expression,
                    context,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
                })?
                else {
                    return Err(attach_primary_span_if_absent(
                        unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                    sources,
                    statement.span(),
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
            "IR v0 can only lower non-terminal `if`/`while` statements for {subject} when branches contain supported local bindings, branch/body-local assignments or explicit aggregate drops, void call statements, returns, or nested non-terminal `if`/`while` statements"
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
