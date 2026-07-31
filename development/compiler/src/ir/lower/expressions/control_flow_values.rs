use super::*;

pub(super) fn lower_i32_if_expression_to_location(
    statement: &IfStmt,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_i32_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_i32_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: I32Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        "E8008",
        |statement, context, then_prologue, else_prologue| {
            lower_i32_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_i32_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: I32Location,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_i32_expression_to_location(expression, destination, branch_context)
        },
    )
}

pub(super) fn lower_u8_if_expression_to_location(
    statement: &IfStmt,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_u8_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_u8_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: U8Location,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        "E8008",
        |statement, context, then_prologue, else_prologue| {
            lower_u8_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_u8_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: U8Location,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_u8_expression_to_location(expression, destination, branch_context)
        },
    )
}

pub(super) fn lower_usize_if_expression_to_location(
    statement: &IfStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_usize_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_usize_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        "E8008",
        |statement, context, then_prologue, else_prologue| {
            lower_usize_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_usize_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_usize_expression_to_location(expression, destination, branch_context)
        },
    )
}

pub(super) fn lower_bool_if_expression_to_location(
    statement: &IfStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        diagnostic_code,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_bool_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        diagnostic_code,
        |statement, context, then_prologue, else_prologue| {
            lower_bool_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                diagnostic_code,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_bool_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_bool_expression_to_location(
                expression,
                destination,
                branch_context,
                diagnostic_code,
            )
        },
    )
}

pub(super) fn lower_str_if_expression_to_location(
    statement: &IfStmt,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_str_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_str_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        "E8008",
        |statement, context, then_prologue, else_prologue| {
            lower_str_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_str_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: StrLocation,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_str_expression_to_location(expression, destination, branch_context)
        },
    )
}

pub(super) fn lower_slice_if_expression_to_location(
    statement: &IfStmt,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_slice_if_expression_to_location_with_branch_prologues(
        statement,
        destination,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
    )
}

pub(super) fn lower_slice_if_is_expression_to_location(
    statement: &IfIsStmt,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_is_expression_to_location(
        statement,
        context,
        "E8008",
        |statement, context, then_prologue, else_prologue| {
            lower_slice_if_expression_to_location_with_branch_prologues(
                statement,
                destination,
                context,
                then_prologue,
                else_prologue,
            )
        },
    )
}

pub(super) fn lower_slice_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    destination: SliceLocation,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_if_expression_to_location_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        |expression, branch_context| {
            lower_slice_expression_to_location(expression, destination, branch_context)
        },
    )
}

pub(super) fn lower_i32_match_expression_to_location(
    statement: &SwitchStmt,
    destination: I32Location,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |expression, switch_context| {
            lower_i32_expression_to_location(expression, destination, switch_context)
        },
    )
}

pub(super) fn lower_u8_match_expression_to_location(
    statement: &SwitchStmt,
    destination: U8Location,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |expression, switch_context| {
            lower_u8_expression_to_location(expression, destination, switch_context)
        },
    )
}

pub(super) fn lower_usize_match_expression_to_location(
    statement: &SwitchStmt,
    destination: UsizeLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |expression, switch_context| {
            lower_usize_expression_to_location(expression, destination, switch_context)
        },
    )
}

pub(super) fn lower_bool_match_expression_to_location(
    statement: &SwitchStmt,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        diagnostic_code,
        |expression, switch_context| {
            lower_bool_expression_to_location(
                expression,
                destination,
                switch_context,
                diagnostic_code,
            )
        },
    )
}

pub(super) fn lower_str_match_expression_to_location(
    statement: &SwitchStmt,
    destination: StrLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |expression, switch_context| {
            lower_str_expression_to_location(expression, destination, switch_context)
        },
    )
}

pub(super) fn lower_slice_match_expression_to_location(
    statement: &SwitchStmt,
    destination: SliceLocation,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_match_expression_to_location(
        statement,
        context,
        reserved_local_abi_words,
        "E8008",
        |expression, switch_context| {
            lower_slice_expression_to_location(expression, destination, switch_context)
        },
    )
}

pub(super) fn lower_match_expression_to_location(
    statement: &SwitchStmt,
    context: &LoweringContext,
    reserved_local_abi_words: usize,
    diagnostic_code: &'static str,
    lower_result: impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut switch_context = context.with_reserved_local_abi_words(reserved_local_abi_words);
    let switch = tag_only_switch_as_control_flow(statement, &mut switch_context, diagnostic_code)?;
    let target_cleanup = switch.target_cleanup;
    let mut instructions = switch.leading_instructions;
    instructions.extend(match switch.body {
        LoweredPayloadlessSwitchBody::Direct(block) => {
            lower_match_switch_block_to_location(block, &switch_context, &lower_result)?
        }
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_match_switch_condition_to_location(condition, &switch_context, &lower_result)?
        }
    });
    if let Some(cleanup) = target_cleanup {
        cleanup.append_to(&mut instructions, &mut switch_context)?;
    }
    Ok(instructions)
}

pub(super) fn lower_match_switch_condition_to_location(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    lower_result: &impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let condition_value = lower_bool_expression_to_value(&condition.condition, context, "E8008")?;
    let mut instructions = condition_value.instructions;
    instructions.push(Instruction::If {
        condition: condition_value.value,
        then_instructions: lower_match_switch_block_to_location(
            condition.then_branch,
            context,
            lower_result,
        )?,
        else_instructions: lower_match_switch_body_to_location(
            *condition.else_body,
            context,
            lower_result,
        )?,
    });
    Ok(instructions)
}

pub(super) fn lower_match_switch_body_to_location(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    lower_result: &impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => {
            lower_match_switch_block_to_location(block, context, lower_result)
        }
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_match_switch_condition_to_location(condition, context, lower_result)
        }
    }
}

pub(super) fn lower_match_switch_block_to_location(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    lower_result: &impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_value_control_block_to_location_with_prologue(
        &block.block,
        context,
        &block.prologue,
        lower_result,
    )
}

pub(super) fn lower_i32_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_i32()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredI32Value {
        instructions: lower_i32_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: I32Value::Location(temporary),
    })
}

pub(super) fn lower_i32_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_i32()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredI32Value {
        instructions: lower_i32_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: I32Value::Location(temporary),
    })
}

pub(super) fn lower_u8_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_u8()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredU8Value {
        instructions: lower_u8_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: U8Value::Location(temporary),
    })
}

pub(super) fn lower_u8_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_u8()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredU8Value {
        instructions: lower_u8_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: U8Value::Location(temporary),
    })
}

pub(super) fn lower_usize_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_usize()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredUsizeValue {
        instructions: lower_usize_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: UsizeValue::Location(temporary),
    })
}

pub(super) fn lower_usize_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_usize()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredUsizeValue {
        instructions: lower_usize_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: UsizeValue::Location(temporary),
    })
}

pub(super) fn lower_bool_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredBoolValue {
        instructions: lower_bool_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
            diagnostic_code,
        )?,
        value: BoolValue::Location(temporary),
    })
}

pub(super) fn lower_bool_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredBoolValue {
        instructions: lower_bool_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
            diagnostic_code,
        )?,
        value: BoolValue::Location(temporary),
    })
}

pub(super) fn lower_str_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredStrValue {
        instructions: lower_str_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: StrValue::Location(temporary),
    })
}

pub(super) fn lower_str_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredStrValue {
        instructions: lower_str_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: StrValue::Location(temporary),
    })
}

pub(super) fn lower_slice_if_expression_to_value(
    statement: &IfStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredSliceValue {
        instructions: lower_slice_if_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: SliceValue::Location(temporary),
    })
}

pub(super) fn lower_slice_if_is_expression_to_value(
    statement: &IfIsStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    let expression_context =
        context.with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
    Ok(LoweredSliceValue {
        instructions: lower_slice_if_is_expression_to_location(
            statement,
            temporary,
            &expression_context,
        )?,
        value: SliceValue::Location(temporary),
    })
}

pub(super) fn lower_i32_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_i32()?;
    Ok(LoweredI32Value {
        instructions: lower_i32_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: I32Value::Location(temporary),
    })
}

pub(super) fn lower_u8_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    let temporary = temporaries.next_u8()?;
    Ok(LoweredU8Value {
        instructions: lower_u8_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: U8Value::Location(temporary),
    })
}

pub(super) fn lower_usize_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_usize()?;
    Ok(LoweredUsizeValue {
        instructions: lower_usize_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: UsizeValue::Location(temporary),
    })
}

pub(super) fn lower_bool_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    Ok(LoweredBoolValue {
        instructions: lower_bool_match_expression_to_location(
            statement,
            temporary,
            context,
            diagnostic_code,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: BoolValue::Location(temporary),
    })
}

pub(super) fn lower_str_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    Ok(LoweredStrValue {
        instructions: lower_str_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: StrValue::Location(temporary),
    })
}

pub(super) fn lower_slice_match_expression_to_value(
    statement: &SwitchStmt,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    Ok(LoweredSliceValue {
        instructions: lower_slice_match_expression_to_location(
            statement,
            temporary,
            context,
            temporaries.reserved_local_abi_words(context)?,
        )?,
        value: SliceValue::Location(temporary),
    })
}

pub(super) fn lower_if_is_expression_to_location(
    statement: &IfIsStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    lower_statement: impl Fn(
        &IfStmt,
        &LoweringContext,
        &BranchPrologue,
        &BranchPrologue,
    ) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut if_context = context.clone();
    let if_is = tag_only_if_is_as_control_flow(statement, &mut if_context, diagnostic_code)?;
    let target_cleanup = if_is.target_cleanup;
    let mut instructions = if_is.leading_instructions;
    instructions.extend(lower_statement(
        &if_is.statement,
        &if_context,
        &if_is.then_prologue,
        &BranchPrologue::empty(),
    )?);
    if let Some(cleanup) = target_cleanup {
        cleanup.append_to(&mut instructions, &mut if_context)?;
    }
    Ok(instructions)
}

pub(super) fn lower_if_expression_to_location_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    lower_result: impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_value_control_expression_diagnostic());
    };
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(unsupported_value_control_expression_diagnostic());
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, "E8008")?;
    let mut instructions = condition.instructions;
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions: lower_value_control_block_to_location_with_prologue(
            &statement.then_block,
            context,
            then_prologue,
            &lower_result,
        )?,
        else_instructions: lower_value_control_block_to_location_with_prologue(
            else_block,
            context,
            else_prologue,
            &lower_result,
        )?,
    });
    Ok(instructions)
}

pub(super) fn lower_value_control_block_to_location_with_prologue(
    block: &Block,
    context: &LoweringContext,
    prologue: &BranchPrologue,
    lower_result: &impl Fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(result) = block.result.as_deref() else {
        return Err(unsupported_value_control_expression_diagnostic());
    };
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let mut instructions = prologue.apply(&mut branch_context)?;
    let (leading_instructions, ends_execution) =
        lower_value_control_leading_statements(&block.statements, &mut branch_context, local_mark)?;
    instructions.extend(leading_instructions);
    if ends_execution {
        return Ok(instructions);
    }
    if expression_contains_explicit_aggregate_move_outside(result, &branch_context, local_mark) {
        return Err(unsupported_value_control_expression_diagnostic());
    }
    instructions.extend(lower_result(result, &branch_context)?);
    mark_explicit_moves_in_expression(result, &mut branch_context);
    instructions.extend(lower_scope_end_drops_for_locals_since(
        &mut branch_context,
        local_mark,
    )?);
    Ok(instructions)
}

pub(super) fn lower_value_control_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    local_mark: usize,
) -> Result<(Vec<Instruction>, bool), Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.value,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(unsupported_value_control_expression_diagnostic());
                }
                if let Some(terminating_instructions) =
                    lower_never_expression_with_scope_drops(&statement.expression, context)?
                {
                    instructions.extend(terminating_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, context);
                    return Ok((instructions, true));
                }
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_value_control_expression_diagnostic());
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(_)
            | Stmt::Return(_)
            | Stmt::If(_)
            | Stmt::IfIs(_)
            | Stmt::Switch(_)
            | Stmt::ForRange(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
            | Stmt::Break(_)
            | Stmt::Continue(_) => return Err(unsupported_value_control_expression_diagnostic()),
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok((instructions, false))
}
