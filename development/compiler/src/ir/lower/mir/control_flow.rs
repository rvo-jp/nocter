//! Structural queries over checked scalar MIR control flow.

use crate::mir::{Body, CallContinuation, Terminator};

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
            Terminator::Switch { .. } => {
                return Err("nested conditional branches require recursive structuring");
            }
            Terminator::Return => return Err("conditional branch returns before its common join"),
        }
    }
}
