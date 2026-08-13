//! Structural queries over checked scalar MIR control flow.

use crate::mir::{Body, CallContinuation, Terminator};

pub(super) fn linear_path_target(
    body: &Body,
    start: crate::mir::BasicBlockId,
) -> Option<crate::mir::BasicBlockId> {
    let mut current = start;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let block = body.blocks.get(current.index())?;
        match &block.terminator {
            Terminator::Goto { target } => return Some(*target),
            Terminator::Drop { target, .. } => current = *target,
            Terminator::Call {
                continuation: CallContinuation::Return { target, .. },
                ..
            } => current = *target,
            Terminator::Call {
                continuation:
                    CallContinuation::Outcome {
                        success, failure, ..
                    },
                ..
            } if dedicated_outcome_failure(body, *failure) => current = *success,
            _ => return None,
        }
    }
}

pub(super) fn conditional_join(
    then_end: crate::mir::BasicBlockId,
    else_end: crate::mir::BasicBlockId,
    loop_header: crate::mir::BasicBlockId,
    loop_exit: crate::mir::BasicBlockId,
) -> Option<crate::mir::BasicBlockId> {
    if then_end == else_end {
        return Some(then_end);
    }
    let is_loop_exit = |target| target == loop_header || target == loop_exit;
    match (is_loop_exit(then_end), is_loop_exit(else_end)) {
        (true, false) => Some(else_end),
        (false, true) => Some(then_end),
        _ => None,
    }
}

fn dedicated_outcome_failure(body: &Body, failure: crate::mir::BasicBlockId) -> bool {
    body.blocks.get(failure.index()).is_some_and(|block| {
        block.statements.is_empty()
            && matches!(
                block.terminator,
                Terminator::Trap | Terminator::PropagateFailure
            )
    })
}

pub(super) fn linear_branch_join(
    body: &Body,
    start: crate::mir::BasicBlockId,
) -> Result<crate::mir::BasicBlockId, &'static str> {
    let mut current = start;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err("conditional branch contains a cycle before its join");
        }
        let block = &body.blocks[current.index()];
        match &block.terminator {
            Terminator::Goto { target } => return Ok(*target),
            Terminator::Drop { .. } => {
                return Err("drop cleanup has not been projected to machine IR");
            }
            Terminator::Call {
                continuation: CallContinuation::Return { target, .. },
                ..
            } => current = *target,
            Terminator::Call {
                continuation: CallContinuation::Never,
                ..
            } => return Err("non-returning conditional branches do not have a common join"),
            Terminator::Call {
                continuation: CallContinuation::Outcome { .. },
                ..
            } => return Err("outcome calls require explicit failure-path structuring"),
            Terminator::Switch { .. } => {
                return Err("nested conditional branches require recursive structuring");
            }
            Terminator::Trap => return Err("conditional branch traps before its common join"),
            Terminator::PropagateFailure => {
                return Err("conditional branch propagates failure before its common join");
            }
            Terminator::Return => return Err("conditional branch returns before its common join"),
        }
    }
}
