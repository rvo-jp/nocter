//! Path-sensitive definite-initialization validation for MIR locals.

use super::dataflow::LocalSet;
use super::{BasicBlockId, Body, CallContinuation, LocalId, LocalStorage, Operand, Rvalue};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InitializationLocation {
    Statement(usize),
    Switch,
    CallArgument(usize),
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InitializationError {
    pub(crate) block: BasicBlockId,
    pub(crate) location: InitializationLocation,
    pub(crate) local: LocalId,
}

pub(super) fn validate(body: &Body) -> Vec<InitializationError> {
    if body.blocks.get(body.entry.index()).is_none() {
        return Vec::new();
    }
    let mut initial = LocalSet::new(body.locals.len());
    for (index, local) in body.locals.iter().enumerate() {
        if matches!(local.storage, LocalStorage::Parameter(_)) {
            initial.insert(LocalId::from_index(index));
        }
    }
    let mut entries = vec![None; body.blocks.len()];
    entries[body.entry.index()] = Some(initial);
    let mut queue = VecDeque::from([body.entry]);
    let mut errors = HashSet::new();

    while let Some(block_id) = queue.pop_front() {
        let Some(block) = body.blocks.get(block_id.index()) else {
            continue;
        };
        let Some(mut initialized) = entries[block_id.index()].clone() else {
            continue;
        };
        for (statement_index, statement) in block.statements.iter().enumerate() {
            let crate::mir::Statement::Assign {
                destination, value, ..
            } = statement;
            for operand in rvalue_operands(value) {
                validate_operand(
                    operand,
                    &initialized,
                    block_id,
                    InitializationLocation::Statement(statement_index),
                    body.locals.len(),
                    &mut errors,
                );
            }
            initialized.insert(destination.local);
        }

        match &block.terminator {
            crate::mir::Terminator::Goto { target } => {
                merge_entry(&mut entries, &mut queue, *target, initialized);
            }
            crate::mir::Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                validate_operand(
                    condition,
                    &initialized,
                    block_id,
                    InitializationLocation::Switch,
                    body.locals.len(),
                    &mut errors,
                );
                merge_entry(&mut entries, &mut queue, *then_target, initialized.clone());
                merge_entry(&mut entries, &mut queue, *else_target, initialized);
            }
            crate::mir::Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                for (index, argument) in arguments.iter().enumerate() {
                    validate_operand(
                        &argument.operand,
                        &initialized,
                        block_id,
                        InitializationLocation::CallArgument(index),
                        body.locals.len(),
                        &mut errors,
                    );
                }
                match continuation {
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        initialized.insert(destination.local);
                        merge_entry(&mut entries, &mut queue, *target, initialized);
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => {
                        let failure_state = initialized.clone();
                        initialized.insert(destination.local);
                        merge_entry(&mut entries, &mut queue, *success, initialized);
                        merge_entry(&mut entries, &mut queue, *failure, failure_state);
                    }
                    CallContinuation::Never => {}
                }
            }
            crate::mir::Terminator::Return => {
                if body.locals.get(body.return_local.index()).is_some()
                    && !initialized.contains(body.return_local)
                {
                    errors.insert(InitializationError {
                        block: block_id,
                        location: InitializationLocation::Return,
                        local: body.return_local,
                    });
                }
            }
            crate::mir::Terminator::Trap | crate::mir::Terminator::PropagateFailure => {}
        }
    }

    let mut errors = errors.into_iter().collect::<Vec<_>>();
    errors.sort_by_key(|error| {
        (
            error.block.index(),
            location_order(error.location),
            error.local.index(),
        )
    });
    errors
}

fn merge_entry(
    entries: &mut [Option<LocalSet>],
    queue: &mut VecDeque<BasicBlockId>,
    target: BasicBlockId,
    incoming: LocalSet,
) {
    let Some(entry) = entries.get_mut(target.index()) else {
        return;
    };
    let changed = match entry {
        None => {
            *entry = Some(incoming);
            true
        }
        Some(existing) => existing.intersect_with(&incoming),
    };
    if changed {
        queue.push_back(target);
    }
}

fn rvalue_operands(value: &Rvalue) -> impl Iterator<Item = &Operand> {
    let operands = match value {
        Rvalue::Use(operand) => [Some(operand), None],
        Rvalue::Binary { left, right, .. } | Rvalue::Compare { left, right, .. } => {
            [Some(left), Some(right)]
        }
    };
    operands.into_iter().flatten()
}

fn validate_operand(
    operand: &Operand,
    initialized: &LocalSet,
    block: BasicBlockId,
    location: InitializationLocation,
    local_count: usize,
    errors: &mut HashSet<InitializationError>,
) {
    if let Operand::Copy(place) = operand
        && place.local.index() < local_count
        && !initialized.contains(place.local)
    {
        errors.insert(InitializationError {
            block,
            location,
            local: place.local,
        });
    }
}

fn location_order(location: InitializationLocation) -> usize {
    match location {
        InitializationLocation::Statement(index) => index,
        InitializationLocation::Switch => usize::MAX - 2,
        InitializationLocation::CallArgument(index) => usize::MAX / 2 + index,
        InitializationLocation::Return => usize::MAX,
    }
}
