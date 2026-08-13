//! Structuring of checked natural-loop MIR into machine-IR loops.

use super::control_flow;
use super::{
    lower_bool_operand, lower_call_argument, lower_call_target, lower_outcome_call,
    lower_returning_call, lower_statements, outcome_failure_mode,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};
use crate::mir::{Body, CallContinuation, Terminator};
use crate::resolve::ResolveOutput;
use crate::source::SourceId;
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_linear_loop_condition(
    body: &Body,
    start: crate::mir::BasicBlockId,
    condition_block: crate::mir::BasicBlockId,
    resolved: &ResolveOutput,
    function_signatures: &super::super::context::FunctionSignatures,
    function_names: &super::super::context::FunctionNames,
    root_source: SourceId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<(Vec<Instruction>, BoolValue), Vec<Diagnostic>> {
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
            return Ok((instructions, lower_bool_operand(condition, body)?));
        }
        current = lower_linear_call_terminator(
            body,
            &block.terminator,
            resolved,
            function_signatures,
            function_names,
            root_source,
            visited,
            &mut instructions,
        )?;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_linear_loop_body(
    body: &Body,
    start: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
    exit: crate::mir::BasicBlockId,
    body_exit: control_flow::LoopBodyExit,
    resolved: &ResolveOutput,
    function_signatures: &super::super::context::FunctionSignatures,
    function_names: &super::super::context::FunctionNames,
    root_source: SourceId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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
            Terminator::Goto { target } if *target == header => {
                if body_exit != control_flow::LoopBodyExit::Backedge {
                    return Err(super::invalid_mir_diagnostics(
                        "loop body exit changed during MIR structuring",
                    ));
                }
                return Ok(instructions);
            }
            Terminator::Goto { target } if *target == exit => {
                if body_exit != control_flow::LoopBodyExit::Break {
                    return Err(super::invalid_mir_diagnostics(
                        "loop body exit changed during MIR structuring",
                    ));
                }
                instructions.push(Instruction::Break);
                return Ok(instructions);
            }
            Terminator::Call { .. } => {
                current = lower_linear_call_terminator(
                    body,
                    &block.terminator,
                    resolved,
                    function_signatures,
                    function_names,
                    root_source,
                    visited,
                    &mut instructions,
                )?;
            }
            _ => {
                return Err(super::invalid_mir_diagnostics(
                    "loop body does not follow a linear path to its backedge or exit",
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_linear_call_terminator(
    body: &Body,
    terminator: &Terminator,
    resolved: &ResolveOutput,
    function_signatures: &super::super::context::FunctionSignatures,
    function_names: &super::super::context::FunctionNames,
    root_source: SourceId,
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
    let (call_target, callee_name) =
        lower_call_target(*callee, resolved, function_names, root_source)?;
    let arguments = arguments
        .iter()
        .map(|argument| lower_call_argument(argument, body))
        .collect::<Result<Vec<_>, _>>()?;
    match continuation {
        CallContinuation::Return {
            destination,
            target,
        } => {
            let scalar = body.locals[destination.local.index()].scalar;
            instructions.push(lower_returning_call(
                body,
                scalar,
                destination,
                call_target,
                arguments,
                &callee_name,
                function_signatures,
            )?);
            Ok(*target)
        }
        CallContinuation::Outcome {
            destination,
            success,
            failure,
        } => {
            let scalar = body.locals[destination.local.index()].scalar;
            instructions.push(lower_outcome_call(
                body,
                scalar,
                destination,
                call_target,
                arguments,
                outcome_failure_mode(body, *failure)?,
                &callee_name,
                function_signatures,
            )?);
            visited.insert(*failure);
            Ok(*success)
        }
        CallContinuation::Never => Err(super::invalid_mir_diagnostics(
            "linear control-flow path contains a non-returning call",
        )),
    }
}
