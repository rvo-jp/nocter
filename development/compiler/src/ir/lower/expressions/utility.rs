use super::*;

pub(super) fn lower_short_circuit_bool_expression_to_location(
    binary: &BinaryExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_expression_to_branch(
        &Expr::Binary(binary.clone()),
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(true),
        }],
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(false),
        }],
        context,
        diagnostic_code,
    )
}

pub(super) fn lower_short_circuit_bool_expression_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let temporary = temporaries.next_bool()?;
    Ok(LoweredBoolValue {
        instructions: lower_short_circuit_bool_expression_to_location_with_temporaries(
            binary,
            temporary,
            context,
            diagnostic_code,
            temporaries,
        )?,
        value: BoolValue::Location(temporary),
    })
}

pub(super) fn lower_short_circuit_bool_expression_to_location_with_temporaries(
    binary: &BinaryExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_bool_expression_to_branch_with_temporaries(
        &Expr::Binary(binary.clone()),
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(true),
        }],
        vec![Instruction::SetBool {
            destination,
            value: BoolValue::Const(false),
        }],
        context,
        diagnostic_code,
        temporaries,
    )
}

pub(super) fn lower_short_circuit_bool_expression_to_branch(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => lower_bool_expression_to_branch(
            &binary.left,
            lower_bool_expression_to_branch(
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
        BinaryOperator::LogicalOr => lower_bool_expression_to_branch(
            &binary.left,
            then_instructions.clone(),
            lower_bool_expression_to_branch(
                &binary.right,
                then_instructions,
                else_instructions,
                context,
                diagnostic_code,
            )?,
            context,
            diagnostic_code,
        ),
        _ => unreachable!("short-circuit bool expression must be && or ||"),
    }
}

pub(super) fn lower_short_circuit_bool_expression_to_branch_with_temporaries(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => {
            let left = lower_bool_expression_to_value_with_temporaries(
                &binary.left,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let then_instructions = lower_bool_expression_to_branch_with_temporaries(
                &binary.right,
                then_instructions,
                else_instructions.clone(),
                context,
                diagnostic_code,
                temporaries,
            )?;
            let mut instructions = left.instructions;
            instructions.push(Instruction::If {
                condition: left.value,
                then_instructions,
                else_instructions,
            });
            Ok(instructions)
        }
        BinaryOperator::LogicalOr => {
            let left = lower_bool_expression_to_value_with_temporaries(
                &binary.left,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let else_instructions = lower_bool_expression_to_branch_with_temporaries(
                &binary.right,
                then_instructions.clone(),
                else_instructions,
                context,
                diagnostic_code,
                temporaries,
            )?;
            let mut instructions = left.instructions;
            instructions.push(Instruction::If {
                condition: left.value,
                then_instructions,
                else_instructions,
            });
            Ok(instructions)
        }
        _ => unreachable!("short-circuit bool expression must be && or ||"),
    }
}

pub(super) fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn lower_str_comparison_to_value_with_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let operator = str_comparison_operator(binary.operator, diagnostic_code)?;
    let left = lower_str_expression_to_value(&binary.left, context, temporaries)?;
    let right = lower_str_expression_to_value(&binary.right, context, temporaries)?;

    let mut instructions = left.instructions;
    let left = materialize_computed_str_value(left.value, &mut instructions, temporaries)?;
    instructions.extend(right.instructions);
    let right = materialize_computed_str_value(right.value, &mut instructions, temporaries)?;
    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::StrComparison {
            operator,
            left,
            right,
        },
    })
}
