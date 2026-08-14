//! Path-sensitive elaboration of owned aggregate replacement assignments.
//!
//! Source assignment does not distinguish first initialization,
//! reinitialization after a move/drop, and replacement of a live value. MIR
//! ownership state does. This pass inserts a `Drop` edge only for the last
//! case, after right-hand-side evaluation and immediately before the store.

use super::model::BasicBlock;
use super::{BasicBlockId, Body, OwnershipKind, Place, Statement, Terminator, ValueRepresentation};

pub(super) fn materialize(body: &mut Body) {
    let analysis = super::drop_obligations::analyze(body);
    let original_block_count = body.blocks.len();
    for block_index in 0..original_block_count {
        let block_id = BasicBlockId::from_index(block_index);
        let replacements = body.blocks[block_index]
            .statements
            .iter()
            .enumerate()
            .filter_map(|(statement, value)| {
                let Statement::Assign { destination, .. } = value else {
                    return None;
                };
                if !owned_aggregate_place(body, *destination) {
                    return None;
                }
                let live = externally_owned_projection(body, *destination)
                    || analysis.replacement_state(body, block_id, statement, *destination)
                        == super::drop_obligations::ReplacementState::Live;
                live.then_some((statement, *destination))
            })
            .collect::<Vec<_>>();

        for (statement, destination) in replacements.into_iter().rev() {
            split_replacement(body, block_id, statement, destination);
        }
    }
}

fn owned_aggregate_place(body: &Body, place: Place) -> bool {
    if let Some(projection) = place.projection {
        return body
            .projections
            .get(projection.index())
            .is_some_and(|path| {
                path.representation == ValueRepresentation::Aggregate
                    && path.ownership == OwnershipKind::Move
            });
    }
    body.locals.get(place.local.index()).is_some_and(|local| {
        local.representation == ValueRepresentation::Aggregate
            && local.ownership == OwnershipKind::Move
    })
}

fn externally_owned_projection(body: &Body, place: Place) -> bool {
    place.projection.is_some()
        && body
            .locals
            .get(place.local.index())
            .is_some_and(|local| local.ownership == OwnershipKind::Borrowed { readwrite: true })
}

fn split_replacement(body: &mut Body, block: BasicBlockId, statement: usize, destination: Place) {
    let Some(plan) = destination
        .projection
        .and_then(|projection| body.projections[projection.index()].drop_plan)
        .or(body.locals[destination.local.index()].drop_plan)
    else {
        return;
    };
    let tail = BasicBlockId::from_index(body.blocks.len());
    let record = &mut body.blocks[block.index()];
    let statements = record.statements.split_off(statement);
    let terminator = std::mem::replace(
        &mut record.terminator,
        Terminator::Drop {
            place: destination,
            plan,
            target: tail,
        },
    );
    let scope = record.scope;
    body.blocks.push(BasicBlock {
        scope,
        statements,
        terminator,
    });
}
