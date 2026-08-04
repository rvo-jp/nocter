use super::*;

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_aggregate_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    };

    let then_instructions = lower_terminal_aggregate_return_block_with_prologue(
        &statement.then_block,
        context,
        &statement.condition,
        then_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_return_block_with_prologue(
        else_block,
        context,
        &statement.condition,
        else_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_aggregate_switch_block(
            block,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_aggregate_switch_condition(
                condition,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )
        }
    }
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_aggregate_switch_block(
        condition.then_branch,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_payloadless_switch_body(
        *condition.else_body,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, "E8007", "functions", "aggregate")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        "E8007",
        "functions",
        "aggregate",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_terminal_aggregate_result_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &mut branch_context,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            if statement.expression.as_ref().is_some_and(|expression| {
                matches!(
                    unwrap_group(expression),
                    Expr::If(_) | Expr::IfIs(_) | Expr::Match(_)
                )
            }) {
                instructions.extend(lower_terminal_return_statement_with_scope_drops(
                    statement,
                    &mut branch_context,
                    "E8007",
                    "functions",
                    sources,
                )?);
                return Ok(instructions);
            }
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !branch_context.pending_aggregate_drops().is_empty()
            {
                instructions.extend(lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    &mut branch_context,
                )?);
                return Ok(instructions);
            }
            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &branch_context,
            )?;
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                &mut branch_context,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_aggregate_if_statement(
                statement,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is = tag_only_if_is_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = tag_only_switch_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) =
                lower_never_expression(&statement.expression, &mut branch_context)?
            else {
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_aggregate_if_diagnostic(function_name)),
    }
}

pub(in crate::ir::lower::functions) fn lower_terminal_aggregate_result_expression(
    expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_aggregate_if_statement(
            statement,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007")?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007")?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        _ => {
            if let Some(terminating_instructions) = lower_never_expression(expression, context)? {
                mark_explicit_moves_in_expression(expression, context);
                return Ok(terminating_instructions);
            }

            mark_explicit_moves_in_expression(expression, context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !context.pending_aggregate_drops().is_empty()
            {
                return lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    context,
                );
            }

            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                context,
            )?;
            append_scope_end_drops_before_exit(return_instructions, context)
        }
    }
}
