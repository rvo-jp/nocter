use super::*;

pub(in crate::ir::lower) fn instruction_list_ends_execution(instructions: &[Instruction]) -> bool {
    match instructions.last() {
        Some(
            Instruction::Return
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnOptionalNone
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

pub(super) fn lower_i32_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_i32_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(super) fn lower_i32_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "i32")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "i32",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_i32_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
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
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is =
                tag_only_if_is_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_i32_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch =
                tag_only_switch_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_i32_payloadless_switch_body(
                switch.body,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
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

pub(super) fn lower_bool_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_bool_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(super) fn lower_bool_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "bool")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "bool",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_bool_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
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
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is =
                tag_only_if_is_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_bool_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch =
                tag_only_switch_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_bool_payloadless_switch_body(
                switch.body,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
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

fn lower_i32_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_i32_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_i32_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_i32_payloadless_switch_body(
                switch.body,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

fn lower_bool_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_bool_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_bool_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_bool_payloadless_switch_body(
                switch.body,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

pub(super) fn lower_scalar_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_scalar_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )
}

pub(super) fn lower_scalar_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, diagnostic_code, subject, return_label)?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        return_label,
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_scalar_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
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
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is =
                tag_only_if_is_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_scalar_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch =
                tag_only_switch_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_scalar_payloadless_switch_body(
                switch.body,
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
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
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

fn lower_scalar_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_scalar_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_return_expression,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_scalar_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_scalar_payloadless_switch_body(
                switch.body,
                context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

pub(super) fn lower_void_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_void_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(super) fn lower_void_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "void")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "void",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_void_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
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
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is =
                tag_only_if_is_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_void_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch =
                tag_only_switch_as_control_flow(statement, &mut branch_context, diagnostic_code)?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_void_payloadless_switch_body(
                switch.body,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
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

fn lower_void_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_void_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_void_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_control_flow(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_void_payloadless_switch_body(
                switch.body,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => {
            if let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(expression, context)?
            {
                mark_explicit_moves_in_expression(expression, context);
                return Ok(terminating_instructions);
            }

            let Some(mut void_instructions) = lower_void_expression_statement(expression, context)?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "void",
                ));
            };
            mark_explicit_moves_in_expression(expression, context);
            void_instructions.extend(append_scope_end_drops_before_exit(
                vec![success_return_instruction(return_type)],
                context,
            )?);
            Ok(void_instructions)
        }
    }
}

fn lower_implicit_return_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let statement = ReturnStmt {
        span: expression.span(),
        expression: Some(expression.clone()),
    };
    lower_return_statement_with_scope_drops(&statement, context, diagnostic_code)
}
