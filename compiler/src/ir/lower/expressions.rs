use super::literals::lower_i32_literal;
use crate::ast::{CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::Instruction;

pub(super) fn lower_i32_expression(expression: &Expr) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower function calls in tail return position",
        )]),
        Expr::Group(group) => lower_i32_expression(&group.expression),
        _ => lower_i32_literal(expression).map(|value| vec![Instruction::LoadI32Const(value)]),
    }
}

pub(super) fn lower_i32_return_expression(
    expression: &Expr,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => lower_i32_tail_call(call),
        Expr::Group(group) => lower_i32_return_expression(&group.expression),
        _ => {
            let mut instructions = lower_i32_expression(expression)?;
            instructions.push(Instruction::Return);
            Ok(instructions)
        }
    }
}

fn lower_i32_tail_call(call: &CallExpr) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !call.arguments.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower zero-argument function calls",
        )]);
    }

    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(vec![Diagnostic::error(
            "E8006",
            "IR v0 can only lower direct function calls",
        )]);
    };

    Ok(vec![Instruction::TailCall(identifier.name.clone())])
}
