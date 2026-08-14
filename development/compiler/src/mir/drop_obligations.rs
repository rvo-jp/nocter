//! Path-sensitive validation of owned MIR cleanup obligations.
//!
//! A type's drop shape is immutable semantic information. This pass tracks
//! only whether one concrete local currently owns a live obligation. Keeping
//! that state in MIR replaces syntax-shaped cleanup heuristics in lowering.

use super::locals::ValueRepresentation;
use super::places::PlaceState;
use super::{
    BasicBlockId, Body, CallContinuation, LocalId, LocalStorage, Operand, OwnershipKind, Place,
    Rvalue, Terminator,
};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
struct ObligationState {
    may_live: PlaceState,
    must_live: PlaceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DropObligationLocation {
    Assignment(usize),
    Drop,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DropObligationErrorKind {
    Overwrite,
    DropOfInactive,
    DropOfNonOwned,
    LiveAtExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DropObligationError {
    pub(crate) block: BasicBlockId,
    pub(crate) location: DropObligationLocation,
    pub(crate) local: LocalId,
    pub(crate) kind: DropObligationErrorKind,
}

pub(super) fn validate(body: &Body) -> Vec<DropObligationError> {
    if body.blocks.get(body.entry.index()).is_none() {
        return Vec::new();
    }

    let mut initial = PlaceState::new(body);
    for (index, local) in body.locals.iter().enumerate() {
        if matches!(local.storage, LocalStorage::Parameter { .. })
            && local.ownership == OwnershipKind::Move
            && local.representation == ValueRepresentation::Aggregate
        {
            initial.initialize(body, Place::local(LocalId::from_index(index)));
        }
    }

    // Both domains are required. `may_live` prevents overwrites and leaks;
    // `must_live` prevents a cleanup from running on a path that moved the
    // value already.
    let mut entries = vec![None; body.blocks.len()];
    entries[body.entry.index()] = Some(ObligationState {
        may_live: initial.clone(),
        must_live: initial,
    });
    let mut queue = VecDeque::from([body.entry]);
    let mut errors = HashSet::new();

    while let Some(block_id) = queue.pop_front() {
        let Some(block) = body.blocks.get(block_id.index()) else {
            continue;
        };
        let Some(mut state) = entries[block_id.index()].clone() else {
            continue;
        };

        for (statement_index, statement) in block.statements.iter().enumerate() {
            match statement {
                super::Statement::BeginAggregate { .. } => {}
                super::Statement::FinishAggregate { destination, .. } => {
                    finish_destination(body, *destination, &mut state);
                }
                super::Statement::Assign {
                    destination, value, ..
                } => {
                    consume_rvalue_moves(body, value, &mut state);
                    activate_destination(
                        body,
                        *destination,
                        &mut state,
                        block_id,
                        DropObligationLocation::Assignment(statement_index),
                        &mut errors,
                    );
                }
                super::Statement::EnterRegion { .. }
                | super::Statement::ExitRegion { .. }
                | super::Statement::BeginLoan { .. }
                | super::Statement::EndLoan { .. } => {}
            }
        }

        match &block.terminator {
            Terminator::Goto { target } => {
                merge_entry(&mut entries, &mut queue, *target, state, body)
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                consume_operand_move(body, condition, &mut state);
                merge_entry(&mut entries, &mut queue, *then_target, state.clone(), body);
                merge_entry(&mut entries, &mut queue, *else_target, state, body);
            }
            Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                for argument in arguments {
                    consume_operand_move(body, &argument.operand, &mut state);
                }
                match continuation {
                    CallContinuation::Continue { target } => {
                        merge_entry(&mut entries, &mut queue, *target, state, body);
                    }
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        activate_destination(
                            body,
                            *destination,
                            &mut state,
                            block_id,
                            DropObligationLocation::Exit,
                            &mut errors,
                        );
                        merge_entry(&mut entries, &mut queue, *target, state, body);
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                        ..
                    } => {
                        let failure_state = state.clone();
                        activate_destination(
                            body,
                            *destination,
                            &mut state,
                            block_id,
                            DropObligationLocation::Exit,
                            &mut errors,
                        );
                        merge_entry(&mut entries, &mut queue, *success, state, body);
                        merge_entry(&mut entries, &mut queue, *failure, failure_state, body);
                    }
                    CallContinuation::OutcomeEffect {
                        success, failure, ..
                    } => {
                        merge_entry(&mut entries, &mut queue, *success, state.clone(), body);
                        merge_entry(&mut entries, &mut queue, *failure, state, body);
                    }
                    CallContinuation::Never => validate_exit(body, block_id, &state, &mut errors),
                }
            }
            Terminator::InspectOutcome {
                source,
                destination,
                success,
                failure,
                ..
            } => {
                consume_operand_move(body, &Operand::Copy(*source), &mut state);
                let failure_state = state.clone();
                activate_destination(
                    body,
                    *destination,
                    &mut state,
                    block_id,
                    DropObligationLocation::Exit,
                    &mut errors,
                );
                merge_entry(&mut entries, &mut queue, *success, state, body);
                merge_entry(&mut entries, &mut queue, *failure, failure_state, body);
            }
            Terminator::Drop { place, target, .. } => {
                if !owned_local(body, place.local) {
                    errors.insert(DropObligationError {
                        block: block_id,
                        location: DropObligationLocation::Drop,
                        local: place.local,
                        kind: DropObligationErrorKind::DropOfNonOwned,
                    });
                } else if !state.must_live.is_available(body, *place) {
                    errors.insert(DropObligationError {
                        block: block_id,
                        location: DropObligationLocation::Drop,
                        local: place.local,
                        kind: DropObligationErrorKind::DropOfInactive,
                    });
                } else {
                    state.may_live.move_out(body, *place);
                    state.must_live.move_out(body, *place);
                }
                merge_entry(&mut entries, &mut queue, *target, state, body);
            }
            Terminator::Return | Terminator::Trap | Terminator::PropagateFailure => {
                validate_exit(body, block_id, &state, &mut errors);
            }
        }
    }

    let mut errors = errors.into_iter().collect::<Vec<_>>();
    errors.sort_by_key(|error| {
        (
            error.block.index(),
            location_order(error.location),
            error.local.index(),
            error_kind_order(error.kind),
        )
    });
    errors
}

fn finish_destination(body: &Body, place: Place, state: &mut ObligationState) {
    if !owned_place(body, place) {
        return;
    }
    state.may_live.finish_aggregate(body, place);
    state.must_live.finish_aggregate(body, place);
}

fn activate_destination(
    body: &Body,
    place: Place,
    state: &mut ObligationState,
    block: BasicBlockId,
    location: DropObligationLocation,
    errors: &mut HashSet<DropObligationError>,
) {
    if !owned_place(body, place) {
        return;
    }
    if state.may_live.any_available_within(body, place) {
        errors.insert(DropObligationError {
            block,
            location,
            local: place.local,
            kind: DropObligationErrorKind::Overwrite,
        });
    }
    state.may_live.initialize(body, place);
    state.must_live.initialize(body, place);
}

fn validate_exit(
    body: &Body,
    block: BasicBlockId,
    state: &ObligationState,
    errors: &mut HashSet<DropObligationError>,
) {
    for (index, local) in body.locals.iter().enumerate() {
        let id = LocalId::from_index(index);
        if id != body.return_local
            && local.ownership == OwnershipKind::Move
            && local.representation == ValueRepresentation::Aggregate
            && state.may_live.any_available_within(body, Place::local(id))
        {
            errors.insert(DropObligationError {
                block,
                location: DropObligationLocation::Exit,
                local: id,
                kind: DropObligationErrorKind::LiveAtExit,
            });
        }
    }
}

fn owned_local(body: &Body, local: LocalId) -> bool {
    body.locals.get(local.index()).is_some_and(|local| {
        local.ownership == OwnershipKind::Move
            && local.representation == ValueRepresentation::Aggregate
    })
}

fn owned_place(body: &Body, place: Place) -> bool {
    let Some(projection) = place.projection else {
        return owned_local(body, place.local);
    };
    body.projections
        .get(projection.index())
        .is_some_and(|projection| {
            projection.base == place.local
                && projection.ownership == OwnershipKind::Move
                && projection.representation == ValueRepresentation::Aggregate
        })
}

fn consume_rvalue_moves(body: &Body, value: &Rvalue, state: &mut ObligationState) {
    match value {
        Rvalue::Use(operand) => consume_operand_move(body, operand, state),
        Rvalue::Variant { leaves, .. } => {
            for leaf in leaves {
                consume_operand_move(body, &leaf.operand, state);
            }
        }
        Rvalue::Unary { operand, .. } | Rvalue::Cast { operand, .. } => {
            consume_operand_move(body, operand, state);
        }
        Rvalue::Binary { left, right, .. } | Rvalue::Compare { left, right, .. } => {
            consume_operand_move(body, left, state);
            consume_operand_move(body, right, state);
        }
    }
}

fn consume_operand_move(body: &Body, operand: &Operand, state: &mut ObligationState) {
    if let Operand::Move(place) = operand {
        state.may_live.move_out(body, *place);
        state.must_live.move_out(body, *place);
    }
}

fn merge_entry(
    entries: &mut [Option<ObligationState>],
    queue: &mut VecDeque<BasicBlockId>,
    target: BasicBlockId,
    incoming: ObligationState,
    body: &Body,
) {
    let Some(entry) = entries.get_mut(target.index()) else {
        return;
    };
    let changed = match entry {
        None => {
            *entry = Some(incoming);
            true
        }
        Some(existing) => {
            let may_changed = existing.may_live.union_with(&incoming.may_live, body);
            let must_changed = existing.must_live.intersect_with(&incoming.must_live, body);
            may_changed || must_changed
        }
    };
    if changed {
        queue.push_back(target);
    }
}

fn location_order(location: DropObligationLocation) -> usize {
    match location {
        DropObligationLocation::Assignment(index) => index,
        DropObligationLocation::Drop => usize::MAX - 1,
        DropObligationLocation::Exit => usize::MAX,
    }
}

fn error_kind_order(kind: DropObligationErrorKind) -> usize {
    match kind {
        DropObligationErrorKind::Overwrite => 0,
        DropObligationErrorKind::DropOfInactive => 1,
        DropObligationErrorKind::DropOfNonOwned => 2,
        DropObligationErrorKind::LiveAtExit => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, Local, LocalOrigin, Place, ReturnMode, ScalarType, Scope, ScopeId, Statement,
    };
    use crate::semantic::{BodyId, TyId};
    use crate::source::{ByteSpan, SourceId};

    fn body_with_owned_parameter(blocks: Vec<BasicBlock>) -> Body {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let scope = ScopeId::from_index(0);
        let ty = TyId::from_index(0);
        Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            root_scope: scope,
            scopes: vec![Scope::root(span)],
            locals: vec![
                Local::scalar(
                    ty,
                    ScalarType::I32,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    scope,
                ),
                Local::aggregate(
                    ty,
                    OwnershipKind::Move,
                    LocalStorage::Parameter { ordinal: 0 },
                    LocalOrigin::Desugared(span),
                    scope,
                ),
            ],
            entry: BasicBlockId::from_index(0),
            blocks,
            loop_regions: Vec::new(),
            allocation_regions: Vec::new(),
            loans: Vec::new(),
            projections: Vec::new(),
            drop_plans: Vec::new(),
        }
    }

    fn block(terminator: Terminator) -> BasicBlock {
        BasicBlock {
            scope: ScopeId::from_index(0),
            statements: Vec::<Statement>::new(),
            terminator,
        }
    }

    fn owned_place() -> Place {
        Place::local(LocalId::from_index(1))
    }

    #[test]
    fn explicit_drop_releases_owned_parameter_before_exit() {
        let body = body_with_owned_parameter(vec![
            block(Terminator::Drop {
                place: owned_place(),
                plan: crate::mir::DropPlanId::from_index(0),
                target: BasicBlockId::from_index(1),
            }),
            block(Terminator::Return),
        ]);

        assert!(validate(&body).is_empty());
    }

    #[test]
    fn live_owned_parameter_is_rejected_at_exit() {
        let body = body_with_owned_parameter(vec![block(Terminator::Return)]);

        assert_eq!(
            validate(&body),
            vec![DropObligationError {
                block: BasicBlockId::from_index(0),
                location: DropObligationLocation::Exit,
                local: LocalId::from_index(1),
                kind: DropObligationErrorKind::LiveAtExit,
            }]
        );
    }

    #[test]
    fn second_drop_of_the_same_path_is_rejected() {
        let body = body_with_owned_parameter(vec![
            block(Terminator::Drop {
                place: owned_place(),
                plan: crate::mir::DropPlanId::from_index(0),
                target: BasicBlockId::from_index(1),
            }),
            block(Terminator::Drop {
                place: owned_place(),
                plan: crate::mir::DropPlanId::from_index(0),
                target: BasicBlockId::from_index(2),
            }),
            block(Terminator::Return),
        ]);

        assert!(validate(&body).iter().any(|error| {
            error.block == BasicBlockId::from_index(1)
                && error.kind == DropObligationErrorKind::DropOfInactive
        }));
    }

    #[test]
    fn branch_join_retains_a_maybe_live_obligation() {
        let mut body = body_with_owned_parameter(vec![
            block(Terminator::Switch {
                condition: Operand::Constant(crate::mir::Constant {
                    ty: TyId::from_index(1),
                    scalar: ScalarType::Bool,
                    value: 1,
                }),
                then_target: BasicBlockId::from_index(1),
                else_target: BasicBlockId::from_index(2),
            }),
            block(Terminator::Drop {
                place: owned_place(),
                plan: crate::mir::DropPlanId::from_index(0),
                target: BasicBlockId::from_index(3),
            }),
            block(Terminator::Goto {
                target: BasicBlockId::from_index(3),
            }),
            block(Terminator::Return),
        ]);
        // Keep a distinct checked boolean type in the body-local type arena.
        body.locals[0].ty = TyId::from_index(1);

        assert!(validate(&body).iter().any(|error| {
            error.block == BasicBlockId::from_index(3)
                && error.kind == DropObligationErrorKind::LiveAtExit
        }));
    }
}
