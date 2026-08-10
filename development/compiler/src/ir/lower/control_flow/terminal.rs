use super::*;

pub(in crate::ir::lower) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_i32_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_i32_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
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
        lower_i32_return_block_with_prologue(
            &statement.then_block,
            context,
            &statement.condition,
            then_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        lower_i32_return_block_with_prologue(
            else_block,
            context,
            &statement.condition,
            else_prologue,
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

pub(in crate::ir::lower) fn lower_terminal_bool_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_bool_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_bool_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
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
        lower_bool_return_block_with_prologue(
            &statement.then_block,
            context,
            &statement.condition,
            then_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        lower_bool_return_block_with_prologue(
            else_block,
            context,
            &statement.condition,
            else_prologue,
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

pub(in crate::ir::lower) fn lower_terminal_u8_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        "u8",
        lower_u8_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_usize_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        "usize",
        lower_usize_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_str_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        "&str",
        lower_str_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_i32_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_i32_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_bool_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_bool_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

fn lower_terminal_scalar_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_scalar_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_u8_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_switch_block(
        block,
        context,
        return_type,
        diagnostic_code,
        subject,
        "u8",
        lower_u8_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_usize_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_switch_block(
        block,
        context,
        return_type,
        diagnostic_code,
        subject,
        "usize",
        lower_usize_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_str_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_switch_block(
        block,
        context,
        return_type,
        diagnostic_code,
        subject,
        "&str",
        lower_str_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_slice_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_label = match return_type.success_type() {
        Type::Slice { is_readwrite: true } => "&+[T]",
        _ => "&[T]",
    };

    lower_terminal_scalar_switch_block(
        block,
        context,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_slice_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_void_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_void_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(super) fn lower_terminal_i32_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_i32_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_i32_switch_condition(
                condition,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
    }
}

fn lower_terminal_i32_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_i32_switch_block(
        condition.then_branch,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_terminal_i32_payloadless_switch_body(
        *condition.else_body,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_bool_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_bool_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_bool_switch_condition(
                condition,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
    }
}

fn lower_terminal_bool_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_bool_switch_block(
        condition.then_branch,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_terminal_bool_payloadless_switch_body(
        *condition.else_body,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_scalar_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_scalar_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_return_expression,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_scalar_switch_condition(
                condition,
                context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )
        }
    }
}

fn lower_terminal_scalar_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_scalar_switch_block(
        condition.then_branch,
        context,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;
    let else_instructions = lower_terminal_scalar_payloadless_switch_body(
        *condition.else_body,
        context,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_void_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_void_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_void_switch_condition(
                condition,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
    }
}

fn lower_terminal_void_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_void_switch_block(
        condition.then_branch,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_terminal_void_payloadless_switch_body(
        *condition.else_body,
        context,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_scalar_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_slice_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_label = match return_type.success_type() {
        Type::Slice { is_readwrite: true } => "&+[T]",
        _ => "&[T]",
    };

    lower_terminal_scalar_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_slice_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_scalar_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
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
    let then_instructions = lower_scalar_return_block(
        &statement.then_block,
        context,
        &statement.condition,
        then_prologue,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;
    let else_instructions = lower_scalar_return_block(
        else_block,
        context,
        &statement.condition,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_void_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_void_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        diagnostic_code,
        subject,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_void_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
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

    let then_instructions = lower_void_return_block_with_prologue(
        &statement.then_block,
        context,
        &statement.condition,
        then_prologue,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_void_return_block_with_prologue(
        else_block,
        context,
        &statement.condition,
        else_prologue,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(in crate::ir::lower) fn lower_terminal_condition(
    condition: &Expr,
    mut then_instructions: Vec<Instruction>,
    mut else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(binary) = short_circuit_condition_needs_branch(condition, context) {
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
    let moved_slots = aggregate_argument_slots_in_instructions(&instructions);
    remove_condition_moved_aggregate_drops(&mut then_instructions, &moved_slots);
    remove_condition_moved_aggregate_drops(&mut else_instructions, &moved_slots);
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions,
        else_instructions,
    });
    Ok(instructions)
}
