//! Structuring of checked natural-loop MIR into machine-IR loops.

use super::control_flow;
use super::{
    lower_bool_operand, lower_call_argument, lower_call_target, lower_outcome_call,
    lower_returning_call, lower_statements, outcome_failure_mode,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};
use crate::mir::{CallContinuation, Operand, Rvalue, Statement, Terminator};
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
        instructions.extend(lower_statements(context, &block.statements)?);
        if current == condition_block {
            let Terminator::Switch { condition, .. } = &block.terminator else {
                return Err(super::invalid_mir_diagnostics(
                    "loop condition path does not end in a switch",
                ));
            };
            if let Some(value) =
                inline_condition_value(context, condition_block, &block.statements, condition)?
            {
                instructions.pop();
                return Ok((instructions, value));
            }
            return Ok((instructions, lower_bool_operand(condition, context)?));
        }
        current =
            lower_linear_call_terminator(context, &block.terminator, visited, &mut instructions)?;
    }
}

fn inline_condition_value(
    context: &super::BackendContext<'_>,
    condition_block: crate::mir::BasicBlockId,
    statements: &[Statement],
    condition: &Operand,
) -> Result<Option<BoolValue>, Vec<Diagnostic>> {
    let body = context.body;
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
    super::lower_comparison(*operator, left, right, *operand_scalar, context).map(Some)
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
        if current == continue_target {
            instructions.extend(lower_continue_path(context, continue_target, header)?);
            return Ok(instructions);
        }
        if current == exit {
            instructions.push(Instruction::Break);
            return Ok(instructions);
        }
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "loop body reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(context, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == continue_target => {
                instructions.extend(lower_continue_path(context, continue_target, header)?);
                return Ok(instructions);
            }
            Terminator::Goto { target } if *target == exit => {
                instructions.push(Instruction::Break);
                return Ok(instructions);
            }
            Terminator::Goto { target } => current = *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(super::drops::lower_drop(context, *place, *plan)?);
                if *target == continue_target {
                    instructions.extend(lower_continue_path(context, continue_target, header)?);
                    instructions.push(Instruction::Continue);
                    return Ok(instructions);
                }
                if *target == exit {
                    instructions.push(Instruction::Break);
                    return Ok(instructions);
                }
                current = *target;
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
                    condition: lower_bool_operand(condition, context)?,
                    then_instructions,
                    else_instructions,
                });
                current = join;
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(format!(
                    "loop body does not follow a linear path to its backedge or exit: block {current:?} has {:?}",
                    block.terminator
                )));
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
        if current == endpoint {
            if endpoint == continue_target {
                instructions.extend(lower_continue_path(context, continue_target, header)?);
                instructions.push(Instruction::Continue);
            } else if endpoint == exit {
                instructions.push(Instruction::Break);
            }
            return Ok(instructions);
        }
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "loop conditional branch reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(context, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == endpoint => {
                if endpoint == continue_target {
                    instructions.extend(lower_continue_path(context, continue_target, header)?);
                    instructions.push(Instruction::Continue);
                } else if endpoint == exit {
                    instructions.push(Instruction::Break);
                }
                return Ok(instructions);
            }
            Terminator::Goto { target } => current = *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(super::drops::lower_drop(context, *place, *plan)?);
                if *target == endpoint {
                    if endpoint == continue_target {
                        instructions.extend(lower_continue_path(context, continue_target, header)?);
                        instructions.push(Instruction::Continue);
                    } else if endpoint == exit {
                        instructions.push(Instruction::Break);
                    }
                    return Ok(instructions);
                }
                current = *target;
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
    context: &super::BackendContext<'_>,
    continue_target: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    if continue_target == header {
        return Ok(Vec::new());
    }
    let block = &body.blocks[continue_target.index()];
    if block.terminator != (Terminator::Goto { target: header }) {
        return Err(super::invalid_mir_diagnostics(
            "loop continue target does not lead to its header",
        ));
    }
    lower_statements(context, &block.statements)
}

fn lower_linear_call_terminator(
    context: &super::BackendContext<'_>,
    terminator: &Terminator,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
    instructions: &mut Vec<Instruction>,
) -> Result<crate::mir::BasicBlockId, Vec<Diagnostic>> {
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
        callee,
        context.resolved,
        context.typed_hir,
        context.function_names,
        context.root_source,
    )?;
    let arguments = arguments
        .iter()
        .map(|argument| lower_call_argument(argument, context))
        .collect::<Result<Vec<_>, _>>()?;
    match continuation {
        CallContinuation::Continue { target } => {
            super::validate_effect_call_return_type(
                &call_target,
                &callee_name,
                context.function_signatures,
            )?;
            instructions.push(Instruction::CallVoid {
                target: call_target,
                arguments,
            });
            Ok(*target)
        }
        CallContinuation::Return {
            destination,
            target,
        } => {
            super::reserve_aggregate_destination(context, destination, instructions)?;
            instructions.push(lower_returning_call(
                context,
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
            failure_payload,
        } => {
            super::reserve_aggregate_destination(context, destination, instructions)?;
            instructions.push(lower_outcome_call(
                context,
                destination,
                call_target,
                arguments,
                outcome_failure_mode(context, *failure, *success, *failure_payload, visited)?,
                &callee_name,
            )?);
            Ok(*success)
        }
        CallContinuation::OutcomeEffect {
            success,
            failure,
            failure_payload,
        } => {
            super::validate_outcome_effect_call_return_type(
                &call_target,
                &callee_name,
                context.function_signatures,
            )?;
            instructions.push(Instruction::CallOutcomeVoid {
                target: call_target,
                arguments,
                failure_mode: outcome_failure_mode(
                    context,
                    *failure,
                    *success,
                    *failure_payload,
                    visited,
                )?,
            });
            Ok(*success)
        }
        CallContinuation::Never => Err(super::invalid_mir_diagnostics(
            "linear control-flow path contains a non-returning call",
        )),
    }
}
