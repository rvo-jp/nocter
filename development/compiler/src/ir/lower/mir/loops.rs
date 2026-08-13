//! Structuring of checked natural-loop MIR into machine-IR loops.

use super::control_flow;
use super::{
    lower_bool_operand, lower_call_argument, lower_call_target, lower_outcome_call,
    lower_returning_call, lower_statements, outcome_failure_mode,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};
use crate::mir::{Body, CallContinuation, Operand, Rvalue, Statement, Terminator};
use std::collections::HashSet;

pub(super) fn lower_linear_loop_condition(
    context: &super::BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    condition_block: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<(Vec<Instruction>, BoolValue), Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if current != start && !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "loop condition reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(body, &block.statements)?);
        if current == condition_block {
            let Terminator::Switch { condition, .. } = &block.terminator else {
                return Err(super::invalid_mir_diagnostics(
                    "loop condition path does not end in a switch",
                ));
            };
            if let Some(value) =
                inline_condition_value(body, condition_block, &block.statements, condition)?
            {
                instructions.pop();
                return Ok((instructions, value));
            }
            return Ok((instructions, lower_bool_operand(condition, body)?));
        }
        current =
            lower_linear_call_terminator(context, &block.terminator, visited, &mut instructions)?;
    }
}

fn inline_condition_value(
    body: &Body,
    condition_block: crate::mir::BasicBlockId,
    statements: &[Statement],
    condition: &Operand,
) -> Result<Option<BoolValue>, Vec<Diagnostic>> {
    if super::storage::inlined_loop_condition_local(body, condition_block).is_none() {
        return Ok(None);
    }
    let Operand::Copy(condition_place) = condition else {
        return Ok(None);
    };
    let Some(Statement::Assign {
        destination,
        value:
            Rvalue::Compare {
                operator,
                left,
                right,
                operand_scalar,
                ..
            },
        ..
    }) = statements.last()
    else {
        return Ok(None);
    };
    if destination != condition_place {
        return Ok(None);
    }
    super::lower_comparison(*operator, left, right, *operand_scalar, body).map(Some)
}

pub(super) fn lower_linear_loop_body(
    context: &super::BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
    exit: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "loop body reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(body, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == continue_target => {
                instructions.extend(lower_continue_path(body, continue_target, header)?);
                return Ok(instructions);
            }
            Terminator::Goto { target } if *target == exit => {
                instructions.push(Instruction::Break);
                return Ok(instructions);
            }
            Terminator::Call { .. } => {
                current = lower_linear_call_terminator(
                    context,
                    &block.terminator,
                    visited,
                    &mut instructions,
                )?;
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                let then_end =
                    control_flow::linear_path_target(body, *then_target).ok_or_else(|| {
                        super::invalid_mir_diagnostics(
                            "loop conditional then-branch is not a linear path",
                        )
                    })?;
                let else_end =
                    control_flow::linear_path_target(body, *else_target).ok_or_else(|| {
                        super::invalid_mir_diagnostics(
                            "loop conditional else-branch is not a linear path",
                        )
                    })?;
                let join =
                    control_flow::conditional_join(then_end, else_end, continue_target, exit)
                        .ok_or_else(|| {
                            super::invalid_mir_diagnostics(
                                "loop conditional does not have one continuation path",
                            )
                        })?;
                let then_instructions = lower_loop_branch_path(
                    context,
                    *then_target,
                    then_end,
                    header,
                    continue_target,
                    exit,
                    visited,
                )?;
                let else_instructions = lower_loop_branch_path(
                    context,
                    *else_target,
                    else_end,
                    header,
                    continue_target,
                    exit,
                    visited,
                )?;
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, body)?,
                    then_instructions,
                    else_instructions,
                });
                current = join;
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "loop body does not follow a linear path to its backedge or exit",
                ));
            }
        }
    }
}

fn lower_loop_branch_path(
    context: &super::BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    endpoint: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
    exit: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "loop conditional branch reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(body, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == endpoint => {
                if endpoint == continue_target {
                    instructions.extend(lower_continue_path(body, continue_target, header)?);
                    instructions.push(Instruction::Continue);
                } else if endpoint == exit {
                    instructions.push(Instruction::Break);
                }
                return Ok(instructions);
            }
            Terminator::Call { .. } => {
                current = lower_linear_call_terminator(
                    context,
                    &block.terminator,
                    visited,
                    &mut instructions,
                )?;
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "loop conditional branch does not reach its classified target",
                ));
            }
        }
    }
}

fn lower_continue_path(
    body: &Body,
    continue_target: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if continue_target == header {
        return Ok(Vec::new());
    }
    let block = &body.blocks[continue_target.index()];
    if block.terminator != (Terminator::Goto { target: header }) {
        return Err(super::invalid_mir_diagnostics(
            "loop continue target does not lead to its header",
        ));
    }
    lower_statements(body, &block.statements)
}

fn lower_linear_call_terminator(
    context: &super::BackendContext<'_>,
    terminator: &Terminator,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
    instructions: &mut Vec<Instruction>,
) -> Result<crate::mir::BasicBlockId, Vec<Diagnostic>> {
    let body = context.body;
    let Terminator::Call {
        callee,
        arguments,
        continuation,
        ..
    } = terminator
    else {
        return Err(super::invalid_mir_diagnostics(
            "linear control-flow path expected a call terminator",
        ));
    };
    let (call_target, callee_name) = lower_call_target(
        *callee,
        context.resolved,
        context.function_names,
        context.root_source,
    )?;
    let arguments = arguments
        .iter()
        .map(|argument| lower_call_argument(argument, body))
        .collect::<Result<Vec<_>, _>>()?;
    match continuation {
        CallContinuation::Return {
            destination,
            target,
        } => {
            let scalar = super::local_scalar(body, destination.local)?;
            instructions.push(lower_returning_call(
                body,
                scalar,
                destination,
                call_target,
                arguments,
                &callee_name,
                context.function_signatures,
            )?);
            Ok(*target)
        }
        CallContinuation::Outcome {
            destination,
            success,
            failure,
        } => {
            let scalar = super::local_scalar(body, destination.local)?;
            instructions.push(lower_outcome_call(
                context,
                scalar,
                destination,
                call_target,
                arguments,
                outcome_failure_mode(body, *failure)?,
                &callee_name,
            )?);
            visited.insert(*failure);
            Ok(*success)
        }
        CallContinuation::Never => Err(super::invalid_mir_diagnostics(
            "linear control-flow path contains a non-returning call",
        )),
    }
}
