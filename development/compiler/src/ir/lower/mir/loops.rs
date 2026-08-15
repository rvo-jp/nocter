//! Structuring of checked natural-loop MIR into machine-IR loops.

use super::control_flow;
use super::{
    lower_bool_operand, lower_call_arguments, lower_call_target, lower_outcome_call,
    lower_outcome_intrinsic_call, lower_statements, outcome_failure_mode,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};
use crate::mir::{CallContinuation, Operand, ReturnMode, Rvalue, Statement, Terminator};
use std::collections::HashSet;

pub(super) fn lower_linear_loop_condition(
    context: &super::BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    condition_block: crate::mir::BasicBlockId,
    body_block: crate::mir::BasicBlockId,
    exit_block: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
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
            match &block.terminator {
                Terminator::Switch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => {
                    let condition = if let Some(value) = inline_condition_value(
                        context,
                        condition_block,
                        &block.statements,
                        condition,
                    )? {
                        instructions.pop();
                        value
                    } else {
                        lower_bool_operand(condition, context)?
                    };
                    let success_cleanup =
                        lower_condition_edge(context, *then_target, body_block, visited)?;
                    let failure_cleanup =
                        lower_condition_edge(context, *else_target, exit_block, visited)?;
                    if !success_cleanup.is_empty() || !failure_cleanup.is_empty() {
                        instructions.push(Instruction::If {
                            condition: condition.clone(),
                            then_instructions: success_cleanup,
                            else_instructions: failure_cleanup,
                        });
                    }
                    return Ok((instructions, condition));
                }
                Terminator::InspectOutcome {
                    source,
                    layer: crate::outcomes::OutcomeLayer::Optional,
                    destination,
                    success,
                    failure,
                    failure_payload: None,
                    ..
                } => {
                    super::reserve_aggregate_destination(context, destination, &mut instructions)?;
                    let success_cleanup =
                        lower_condition_edge(context, *success, body_block, visited)?;
                    let failure_cleanup =
                        lower_condition_edge(context, *failure, exit_block, visited)?;
                    let (inspection, condition) = super::outcomes::lower_optional_loop_condition(
                        context,
                        source,
                        *destination,
                        success_cleanup,
                        failure_cleanup,
                    )?;
                    instructions.push(inspection);
                    return Ok((instructions, condition));
                }
                _ => {
                    return Err(super::invalid_mir_diagnostics(
                        "loop condition path does not end in a switch or optional inspection",
                    ));
                }
            }
        }
        current = match &block.terminator {
            Terminator::Goto { target } => *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(super::drops::lower_drop(context, *place, *plan)?);
                *target
            }
            Terminator::Call { .. } => lower_linear_call_terminator(
                context,
                &block.terminator,
                Some(LoopTargets {
                    header: condition_block,
                    continue_target,
                    exit: exit_block,
                }),
                visited,
                &mut instructions,
            )?,
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "loop condition setup is not a linear call or cleanup path",
                ));
            }
        };
    }
}

fn lower_condition_edge(
    context: &super::BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    endpoint: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut current = start;
    while current != endpoint {
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(
                "optional loop condition cleanup reuses a lowered block",
            ));
        }
        let block = &context.body.blocks[current.index()];
        instructions.extend(lower_statements(context, &block.statements)?);
        current = match &block.terminator {
            Terminator::Goto { target } => *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(super::drops::lower_drop(context, *place, *plan)?);
                *target
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "optional loop condition cleanup is not linear",
                ));
            }
        };
    }
    Ok(instructions)
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
        if let Some((nested_loop, nested_exit)) = lower_nested_loop_at(context, current, visited)? {
            instructions.push(nested_loop);
            current = nested_exit;
            continue;
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
                if lower_terminal_call(context, &block.terminator, &mut instructions)? {
                    return Ok(instructions);
                }
                current = lower_linear_call_terminator(
                    context,
                    &block.terminator,
                    Some(LoopTargets {
                        header,
                        continue_target,
                        exit,
                    }),
                    visited,
                    &mut instructions,
                )?;
            }
            terminator if lower_terminal_terminator(context, terminator, &mut instructions)? => {
                return Ok(instructions);
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
                join_target,
            } => {
                let boundaries = [header, continue_target, exit];
                let then_boundary =
                    control_flow::linear_path_boundary(body, *then_target, &boundaries);
                let else_boundary =
                    control_flow::linear_path_boundary(body, *else_target, &boundaries);
                if let (None, Some(then_boundary), Some(else_boundary)) =
                    (*join_target, then_boundary, else_boundary)
                {
                    let then_instructions = lower_loop_branch_path(
                        context,
                        *then_target,
                        then_boundary,
                        header,
                        continue_target,
                        exit,
                        visited,
                    )?;
                    let else_instructions = lower_loop_branch_path(
                        context,
                        *else_target,
                        else_boundary,
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
                    return Ok(instructions);
                }
                let then_end = control_flow::linear_path_target(body, *then_target);
                let else_end = control_flow::linear_path_target(body, *else_target);
                let join = (*join_target).or_else(|| {
                    control_flow::conditional_join(then_end?, else_end?, continue_target, exit)
                });
                let join = join.ok_or_else(|| {
                    super::invalid_mir_diagnostics(
                        "loop conditional does not have a structured continuation",
                    )
                })?;
                let branch_endpoint = |start| {
                    control_flow::linear_path_boundary(
                        body,
                        start,
                        &[join, header, continue_target, exit],
                    )
                    .unwrap_or(join)
                };
                let then_endpoint = branch_endpoint(*then_target);
                let else_endpoint = branch_endpoint(*else_target);
                let then_instructions = lower_loop_branch_path(
                    context,
                    *then_target,
                    then_endpoint,
                    header,
                    continue_target,
                    exit,
                    visited,
                )?;
                let else_instructions = lower_loop_branch_path(
                    context,
                    *else_target,
                    else_endpoint,
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
            } else if endpoint == header {
                instructions.push(Instruction::Continue);
            } else if endpoint == exit {
                instructions.push(Instruction::Break);
            }
            return Ok(instructions);
        }
        if let Some((nested_loop, nested_exit)) = lower_nested_loop_at(context, current, visited)? {
            instructions.push(nested_loop);
            current = nested_exit;
            continue;
        }
        if !visited.insert(current) {
            return Err(super::invalid_mir_diagnostics(format!(
                "loop conditional branch from {start:?} to {endpoint:?} reuses already lowered block {current:?} (header {header:?}, continue {continue_target:?}, exit {exit:?})"
            )));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(context, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == endpoint => {
                if endpoint == continue_target {
                    instructions.extend(lower_continue_path(context, continue_target, header)?);
                    instructions.push(Instruction::Continue);
                } else if endpoint == header {
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
                    } else if endpoint == header {
                        instructions.push(Instruction::Continue);
                    } else if endpoint == exit {
                        instructions.push(Instruction::Break);
                    }
                    return Ok(instructions);
                }
                current = *target;
            }
            Terminator::Call { .. } => {
                if lower_terminal_call(context, &block.terminator, &mut instructions)? {
                    return Ok(instructions);
                }
                current = lower_linear_call_terminator(
                    context,
                    &block.terminator,
                    Some(LoopTargets {
                        header,
                        continue_target,
                        exit,
                    }),
                    visited,
                    &mut instructions,
                )?;
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
                join_target,
            } => {
                let join = join_target.unwrap_or(endpoint);
                let boundaries = [join, header, continue_target, exit];
                let branch_endpoint = |start| {
                    control_flow::linear_path_boundary(body, start, &boundaries).unwrap_or(join)
                };
                let then_instructions = lower_loop_branch_path(
                    context,
                    *then_target,
                    branch_endpoint(*then_target),
                    header,
                    continue_target,
                    exit,
                    visited,
                )?;
                let else_instructions = lower_loop_branch_path(
                    context,
                    *else_target,
                    branch_endpoint(*else_target),
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
            terminator if lower_terminal_terminator(context, terminator, &mut instructions)? => {
                return Ok(instructions);
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "loop conditional branch does not reach its classified target",
                ));
            }
        }
    }
}

fn lower_nested_loop_at(
    context: &super::BackendContext<'_>,
    current: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Option<(Instruction, crate::mir::BasicBlockId)>, Vec<Diagnostic>> {
    let Some(nested) = context
        .body
        .loop_regions
        .iter()
        .find(|loop_| loop_.header == current)
    else {
        return Ok(None);
    };
    if !visited.insert(current) {
        return Err(super::invalid_mir_diagnostics(
            "nested loop header reuses an already lowered block",
        ));
    }
    let (condition_instructions, condition) = lower_linear_loop_condition(
        context,
        nested.header,
        nested.condition,
        nested.body,
        nested.exit,
        nested.continue_target,
        visited,
    )?;
    let body_instructions = lower_linear_loop_body(
        context,
        nested.body,
        nested.header,
        nested.exit,
        nested.continue_target,
        visited,
    )?;
    Ok(Some((
        Instruction::While {
            condition_instructions,
            condition,
            body_instructions,
        },
        nested.exit,
    )))
}

fn lower_terminal_terminator(
    context: &super::BackendContext<'_>,
    terminator: &Terminator,
    instructions: &mut Vec<Instruction>,
) -> Result<bool, Vec<Diagnostic>> {
    if let Terminator::ReturnValue { source } = terminator {
        instructions.extend(super::lower_value_return(context, source)?);
        instructions.push(super::success_return_instruction(context.body));
        return Ok(true);
    }
    let instruction = match terminator {
        Terminator::Return => super::success_return_instruction(context.body),
        Terminator::ReturnOutcome { source } => super::outcomes::lower_return(context, source)?,
        Terminator::ReturnFailure { code, message } => {
            if context.body.return_mode != ReturnMode::Fallible {
                return Err(super::invalid_mir_diagnostics(
                    "plain MIR loop contains a recoverable failure return",
                ));
            }
            Instruction::ReturnFallibleFailure {
                code: super::lower_str_operand(code, context)?,
                message: super::lower_str_operand(message, context)?,
            }
        }
        Terminator::ReturnOutcomeSuccess { source } => {
            instructions.extend(super::lower_outcome_success_return(context, source)?);
            instructions.push(Instruction::ReturnOutcomeSuccess);
            return Ok(true);
        }
        Terminator::ReturnOptionalNone => Instruction::ReturnOptionalNone,
        Terminator::PropagateFailure => {
            if context.body.return_mode == ReturnMode::Fallible {
                Instruction::PropagateFailure
            } else if context
                .body
                .outcome_contract
                .as_ref()
                .is_some_and(|contract| {
                    contract
                        .layers
                        .contains(&crate::outcomes::OutcomeLayer::Optional)
                })
            {
                Instruction::ReturnOptionalNone
            } else {
                return Err(super::invalid_mir_diagnostics(
                    "plain MIR loop propagates a recoverable failure",
                ));
            }
        }
        Terminator::Trap => Instruction::Trap,
        _ => return Ok(false),
    };
    instructions.push(instruction);
    Ok(true)
}

fn lower_terminal_call(
    context: &super::BackendContext<'_>,
    terminator: &Terminator,
    instructions: &mut Vec<Instruction>,
) -> Result<bool, Vec<Diagnostic>> {
    let Terminator::Call {
        callee,
        arguments,
        continuation: CallContinuation::Never,
        ..
    } = terminator
    else {
        return Ok(false);
    };
    let (call_target, callee_name) = lower_call_target(
        callee,
        context.resolved,
        context.typed_hir,
        context.function_names,
        context.root_source,
    )?;
    let arguments = lower_call_arguments(arguments, context)?;
    super::validate_never_call_return_type(
        &call_target,
        &callee_name,
        context.function_signatures,
    )?;
    let fits_tail_call_abi = arguments
        .iter()
        .map(crate::ir::ScalarArgument::abi_word_count)
        .sum::<usize>()
        <= crate::abi::ARGUMENT_REGISTER_COUNT
        && !arguments
            .iter()
            .any(crate::ir::ScalarArgument::requires_current_frame_for_tail_call);
    if fits_tail_call_abi {
        instructions.push(Instruction::TailCall {
            target: call_target,
            arguments,
        });
    } else {
        instructions.push(Instruction::CallVoid {
            target: call_target,
            arguments,
        });
        instructions.push(Instruction::Trap);
    }
    Ok(true)
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
    loop_targets: Option<LoopTargets>,
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
    if let crate::mir::CallableIdentity::Intrinsic(intrinsic) = &callee.callable {
        return match continuation {
            CallContinuation::Outcome {
                destination,
                success,
                failure,
                failure_payload,
            } => {
                super::reserve_aggregate_destination(context, destination, instructions)?;
                instructions.extend(lower_outcome_intrinsic_call(
                    context,
                    *intrinsic,
                    Some(destination),
                    arguments,
                    linear_outcome_failure_mode(
                        context,
                        *failure,
                        *success,
                        *failure_payload,
                        loop_targets,
                        visited,
                    )?,
                )?);
                Ok(*success)
            }
            CallContinuation::OutcomeEffect {
                success,
                failure,
                failure_payload,
            } => {
                instructions.extend(lower_outcome_intrinsic_call(
                    context,
                    *intrinsic,
                    None,
                    arguments,
                    linear_outcome_failure_mode(
                        context,
                        *failure,
                        *success,
                        *failure_payload,
                        loop_targets,
                        visited,
                    )?,
                )?);
                Ok(*success)
            }
            _ => Err(super::invalid_mir_diagnostics(
                "outcome intrinsic has a non-outcome continuation in a loop",
            )),
        };
    }
    let (call_target, callee_name) = lower_call_target(
        callee,
        context.resolved,
        context.typed_hir,
        context.function_names,
        context.root_source,
    )?;
    let arguments = lower_call_arguments(arguments, context)?;
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
            instructions.extend(super::lower_returning_call_sequence(
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
                linear_outcome_failure_mode(
                    context,
                    *failure,
                    *success,
                    *failure_payload,
                    loop_targets,
                    visited,
                )?,
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
                failure_mode: linear_outcome_failure_mode(
                    context,
                    *failure,
                    *success,
                    *failure_payload,
                    loop_targets,
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

#[derive(Clone, Copy)]
struct LoopTargets {
    header: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
    exit: crate::mir::BasicBlockId,
}

fn linear_outcome_failure_mode(
    context: &super::BackendContext<'_>,
    failure: crate::mir::BasicBlockId,
    success: crate::mir::BasicBlockId,
    failure_payload: Option<crate::mir::LocalId>,
    loop_targets: Option<LoopTargets>,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<crate::ir::OutcomeFailureMode, Vec<Diagnostic>> {
    let Some(targets) = loop_targets else {
        return outcome_failure_mode(context, failure, success, failure_payload, visited);
    };
    let boundaries = [targets.continue_target, targets.exit];
    let Some(boundary) = control_flow::linear_path_boundary(context.body, failure, &boundaries)
    else {
        return outcome_failure_mode(context, failure, success, failure_payload, visited);
    };
    let instructions = lower_loop_branch_path(
        context,
        failure,
        boundary,
        targets.header,
        targets.continue_target,
        targets.exit,
        visited,
    )?;
    if let Some(payload) = failure_payload {
        let (code, message) = super::error_locations(context.body, payload)?;
        Ok(crate::ir::OutcomeFailureMode::Catch {
            code,
            message,
            instructions,
            recovers: false,
        })
    } else {
        Ok(crate::ir::OutcomeFailureMode::Handle { instructions })
    }
}
