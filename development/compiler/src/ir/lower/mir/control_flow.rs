//! Structural queries over checked scalar MIR control flow.

use crate::mir::{Body, CallContinuation, Terminator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoopBodyExit {
    Backedge,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LinearLoop {
    pub(super) condition: crate::mir::BasicBlockId,
    pub(super) body: crate::mir::BasicBlockId,
    pub(super) exit: crate::mir::BasicBlockId,
    pub(super) body_exit: LoopBodyExit,
}

pub(super) fn linear_loop(body: &Body, header: crate::mir::BasicBlockId) -> Option<LinearLoop> {
    let mut current = header;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let block = body.blocks.get(current.index())?;
        match &block.terminator {
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
            Terminator::Switch {
                then_target,
                else_target,
                ..
            } => {
                let body_exit = linear_loop_body_exit(body, *then_target, header, *else_target)?;
                return Some(LinearLoop {
                    condition: current,
                    body: *then_target,
                    exit: *else_target,
                    body_exit,
                });
            }
            _ => return None,
        }
    }
}

pub(super) fn linear_loop_body_exit(
    body: &Body,
    start: crate::mir::BasicBlockId,
    header: crate::mir::BasicBlockId,
    exit: crate::mir::BasicBlockId,
) -> Option<LoopBodyExit> {
    let mut current = start;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let block = body.blocks.get(current.index())?;
        match &block.terminator {
            Terminator::Goto { target } if *target == header => {
                return Some(LoopBodyExit::Backedge);
            }
            Terminator::Goto { target } if *target == exit => return Some(LoopBodyExit::Break),
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
