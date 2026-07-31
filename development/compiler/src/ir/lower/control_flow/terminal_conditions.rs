use super::*;

pub(super) fn lower_short_circuit_terminal_condition(
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

pub(super) fn short_circuit_condition_needs_branch<'a>(
    condition: &'a Expr,
    context: &LoweringContext,
) -> Option<&'a BinaryExpr> {
    let condition = unwrap_group(condition);
    let Expr::Binary(binary) = condition else {
        return None;
    };

    if short_circuit_bool_expression_needs_branch(binary, context) {
        Some(binary)
    } else {
        None
    }
}
