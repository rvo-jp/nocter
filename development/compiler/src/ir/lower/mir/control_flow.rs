//! Structural queries over checked scalar MIR control flow.

use crate::mir::{Body, CallContinuation, Terminator};

pub(super) fn structured_join(
    body: &Body,
    then_target: crate::mir::BasicBlockId,
    else_target: crate::mir::BasicBlockId,
    boundary: Option<crate::mir::BasicBlockId>,
) -> Option<crate::mir::BasicBlockId> {
    let then_distances = reachable_distances(body, then_target, boundary);
    let else_distances = reachable_distances(body, else_target, boundary);
    then_distances
        .iter()
        .filter_map(|(block, then_distance)| {
            else_distances
                .get(block)
                .map(|else_distance| (*block, then_distance + else_distance))
        })
        .min_by_key(|(block, distance)| (*distance, block.index()))
        .map(|(block, _)| block)
}

pub(super) fn can_reach(
    body: &Body,
    start: crate::mir::BasicBlockId,
    target: crate::mir::BasicBlockId,
) -> bool {
    reachable_distances(body, start, Some(target)).contains_key(&target)
}

fn reachable_distances(
    body: &Body,
    start: crate::mir::BasicBlockId,
    boundary: Option<crate::mir::BasicBlockId>,
) -> std::collections::HashMap<crate::mir::BasicBlockId, usize> {
    let mut distances = std::collections::HashMap::new();
    let mut queue = std::collections::VecDeque::from([(start, 0)]);
    while let Some((current, distance)) = queue.pop_front() {
        if distances.insert(current, distance).is_some() || Some(current) == boundary {
            continue;
        }
        let Some(block) = body.blocks.get(current.index()) else {
            continue;
        };
        let mut enqueue = |target| queue.push_back((target, distance + 1));
        match &block.terminator {
            Terminator::Goto { target } | Terminator::Drop { target, .. } => enqueue(*target),
            Terminator::Switch {
                then_target,
                else_target,
                ..
            } => {
                enqueue(*then_target);
                enqueue(*else_target);
            }
            Terminator::Call { continuation, .. } => match continuation {
                CallContinuation::Never => {}
                CallContinuation::Continue { target } | CallContinuation::Return { target, .. } => {
                    enqueue(*target)
                }
                CallContinuation::Outcome {
                    success, failure, ..
                }
                | CallContinuation::OutcomeEffect {
                    success, failure, ..
                } => {
                    enqueue(*success);
                    enqueue(*failure);
                }
            },
            Terminator::InspectOutcome {
                success, failure, ..
            } => {
                enqueue(*success);
                enqueue(*failure);
            }
            Terminator::Trap
            | Terminator::PropagateFailure
            | Terminator::ReturnOutcome { .. }
            | Terminator::Return => {}
        }
    }
    distances
}

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
                continuation:
                    CallContinuation::Continue { target } | CallContinuation::Return { target, .. },
                ..
            } => current = *target,
            Terminator::Call {
                continuation:
                    CallContinuation::Outcome {
                        success, failure, ..
                    }
                    | CallContinuation::OutcomeEffect {
                        success, failure, ..
                    },
                ..
            } if dedicated_outcome_failure(body, *failure) => current = *success,
            Terminator::InspectOutcome {
                success, failure, ..
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
