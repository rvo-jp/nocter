//! Path-sensitive definite-initialization validation for MIR locals.

use super::places::PlaceState;
use super::{BasicBlockId, Body, CallContinuation, LocalId, LocalStorage, Operand, Place, Rvalue};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InitializationLocation {
    Statement(usize),
    Switch,
    CallArgument(usize),
    Drop,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InitializationError {
    pub(crate) block: BasicBlockId,
    pub(crate) location: InitializationLocation,
    pub(crate) local: LocalId,
}

pub(super) struct InitializationAnalysis {
    errors: Vec<InitializationError>,
    edge_states: HashMap<(BasicBlockId, BasicBlockId), PlaceState>,
    exit_states: HashMap<BasicBlockId, PlaceState>,
}

impl InitializationAnalysis {
    pub(super) fn initialized_on_edge(
        &self,
        body: &Body,
        from: BasicBlockId,
        to: BasicBlockId,
        place: Place,
    ) -> bool {
        self.edge_states
            .get(&(from, to))
            .is_some_and(|state| state.is_available(body, place))
    }

    pub(super) fn initialized_at_exit(
        &self,
        body: &Body,
        block: BasicBlockId,
        place: Place,
    ) -> bool {
        self.exit_states
            .get(&block)
            .is_some_and(|state| state.is_available(body, place))
    }
}

pub(super) fn validate(body: &Body) -> Vec<InitializationError> {
    analyze(body).errors
}

pub(super) fn analyze(body: &Body) -> InitializationAnalysis {
    if body.blocks.get(body.entry.index()).is_none() {
        return InitializationAnalysis {
            errors: Vec::new(),
            edge_states: HashMap::new(),
            exit_states: HashMap::new(),
        };
    }
    let mut initial = PlaceState::new(body);
    for (index, local) in body.locals.iter().enumerate() {
        if matches!(local.storage, LocalStorage::Parameter { .. }) {
            initial.initialize(body, Place::local(LocalId::from_index(index)));
        }
    }
    let mut entries = vec![None; body.blocks.len()];
    entries[body.entry.index()] = Some(initial);
    let mut queue = VecDeque::from([body.entry]);
    let mut errors = HashSet::new();
    let mut edge_states = HashMap::new();
    let mut exit_states = HashMap::new();

    while let Some(block_id) = queue.pop_front() {
        let Some(block) = body.blocks.get(block_id.index()) else {
            continue;
        };
        let Some(mut initialized) = entries[block_id.index()].clone() else {
            continue;
        };
        for (statement_index, statement) in block.statements.iter().enumerate() {
            match statement {
                crate::mir::Statement::Assign {
                    destination, value, ..
                } => {
                    for operand in rvalue_operands(value) {
                        validate_and_apply_operand(
                            operand,
                            &mut initialized,
                            block_id,
                            InitializationLocation::Statement(statement_index),
                            body,
                            &mut errors,
                        );
                    }
                    initialized.initialize(body, *destination);
                }
                crate::mir::Statement::BeginLoan { loan, .. } => {
                    if let Some(loan) = body.loans.get(loan.index()) {
                        validate_and_apply_operand(
                            &Operand::Copy(loan.source),
                            &mut initialized,
                            block_id,
                            InitializationLocation::Statement(statement_index),
                            body,
                            &mut errors,
                        );
                        initialized.initialize(body, Place::local(loan.destination));
                    }
                }
                crate::mir::Statement::EndLoan { loan } => {
                    if let Some(loan) = body.loans.get(loan.index()) {
                        initialized.move_out(body, Place::local(loan.destination));
                    }
                }
            }
        }

        match &block.terminator {
            crate::mir::Terminator::Goto { target } => {
                edge_states.insert((block_id, *target), initialized.clone());
                merge_entry(&mut entries, &mut queue, *target, initialized, body);
            }
            crate::mir::Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                validate_and_apply_operand(
                    condition,
                    &mut initialized,
                    block_id,
                    InitializationLocation::Switch,
                    body,
                    &mut errors,
                );
                edge_states.insert((block_id, *then_target), initialized.clone());
                edge_states.insert((block_id, *else_target), initialized.clone());
                merge_entry(
                    &mut entries,
                    &mut queue,
                    *then_target,
                    initialized.clone(),
                    body,
                );
                merge_entry(&mut entries, &mut queue, *else_target, initialized, body);
            }
            crate::mir::Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                for (index, argument) in arguments.iter().enumerate() {
                    validate_and_apply_operand(
                        &argument.operand,
                        &mut initialized,
                        block_id,
                        InitializationLocation::CallArgument(index),
                        body,
                        &mut errors,
                    );
                }
                match continuation {
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        initialized.initialize(body, *destination);
                        edge_states.insert((block_id, *target), initialized.clone());
                        merge_entry(&mut entries, &mut queue, *target, initialized, body);
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => {
                        let failure_state = initialized.clone();
                        initialized.initialize(body, *destination);
                        edge_states.insert((block_id, *success), initialized.clone());
                        edge_states.insert((block_id, *failure), failure_state.clone());
                        merge_entry(&mut entries, &mut queue, *success, initialized, body);
                        merge_entry(&mut entries, &mut queue, *failure, failure_state, body);
                    }
                    CallContinuation::Never => {}
                }
            }
            crate::mir::Terminator::Drop { place, target, .. } => {
                validate_and_apply_operand(
                    &Operand::Move(*place),
                    &mut initialized,
                    block_id,
                    InitializationLocation::Drop,
                    body,
                    &mut errors,
                );
                edge_states.insert((block_id, *target), initialized.clone());
                merge_entry(&mut entries, &mut queue, *target, initialized, body);
            }
            crate::mir::Terminator::Return => {
                if body.locals.get(body.return_local.index()).is_some()
                    && !initialized.is_available(body, Place::local(body.return_local))
                {
                    errors.insert(InitializationError {
                        block: block_id,
                        location: InitializationLocation::Return,
                        local: body.return_local,
                    });
                }
                exit_states.insert(block_id, initialized);
            }
            crate::mir::Terminator::Trap | crate::mir::Terminator::PropagateFailure => {
                exit_states.insert(block_id, initialized);
            }
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
    InitializationAnalysis {
        errors,
        edge_states,
        exit_states,
    }
}

fn merge_entry(
    entries: &mut [Option<PlaceState>],
    queue: &mut VecDeque<BasicBlockId>,
    target: BasicBlockId,
    incoming: PlaceState,
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
        Some(existing) => existing.intersect_with(&incoming, body),
    };
    if changed {
        queue.push_back(target);
    }
}

fn rvalue_operands(value: &Rvalue) -> impl Iterator<Item = &Operand> {
    let operands = match value {
        Rvalue::Use(operand) => [Some(operand), None],
        Rvalue::Unary { operand, .. } | Rvalue::Cast { operand, .. } => [Some(operand), None],
        Rvalue::Binary { left, right, .. } | Rvalue::Compare { left, right, .. } => {
            [Some(left), Some(right)]
        }
    };
    operands.into_iter().flatten()
}

fn validate_and_apply_operand(
    operand: &Operand,
    initialized: &mut PlaceState,
    block: BasicBlockId,
    location: InitializationLocation,
    body: &Body,
    errors: &mut HashSet<InitializationError>,
) {
    let place = match operand {
        Operand::Constant(_) => return,
        Operand::Copy(place) | Operand::Move(place) => place,
    };
    if place.local.index() >= body.locals.len() {
        return;
    }
    if !initialized.is_available(body, *place) {
        errors.insert(InitializationError {
            block,
            location,
            local: place.local,
        });
    }
    if matches!(operand, Operand::Move(_)) {
        initialized.move_out(body, *place);
    }
}

fn location_order(location: InitializationLocation) -> usize {
    match location {
        InitializationLocation::Statement(index) => index,
        InitializationLocation::Switch => usize::MAX - 2,
        InitializationLocation::CallArgument(index) => usize::MAX / 2 + index,
        InitializationLocation::Drop => usize::MAX - 1,
        InitializationLocation::Return => usize::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, Constant, Local, LocalOrigin, Origin, Place, ReturnMode, ScalarType, Scope,
        ScopeId, Statement, Terminator,
    };
    use crate::semantic::{BodyId, ExprId, TyId};
    use crate::source::{ByteSpan, SourceId};

    #[test]
    fn a_move_removes_the_source_from_later_state() {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let ty = TyId::from_index(0);
        let root_scope = ScopeId::from_index(0);
        let source = LocalId::from_index(1);
        let body = Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            root_scope,
            scopes: vec![Scope::root(span)],
            locals: vec![
                Local::scalar(
                    ty,
                    ScalarType::I32,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    root_scope,
                ),
                Local::scalar(
                    ty,
                    ScalarType::I32,
                    LocalStorage::Local,
                    LocalOrigin::Desugared(span),
                    root_scope,
                ),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![BasicBlock {
                scope: root_scope,
                statements: vec![
                    Statement::Assign {
                        destination: Place::local(source),
                        value: Rvalue::Use(Operand::Constant(Constant {
                            ty,
                            scalar: ScalarType::I32,
                            value: 1,
                        })),
                        origin: Origin::Expression(ExprId::from_index(0)),
                    },
                    Statement::Assign {
                        destination: Place::local(LocalId::from_index(0)),
                        value: Rvalue::Use(Operand::Move(Place::local(source))),
                        origin: Origin::Expression(ExprId::from_index(1)),
                    },
                    Statement::Assign {
                        destination: Place::local(LocalId::from_index(0)),
                        value: Rvalue::Use(Operand::Move(Place::local(source))),
                        origin: Origin::Expression(ExprId::from_index(2)),
                    },
                ],
                terminator: Terminator::Return,
            }],
            loop_regions: Vec::new(),
            loans: Vec::new(),
            projections: Vec::new(),
            drop_plans: Vec::new(),
        };

        assert_eq!(
            validate(&body),
            vec![InitializationError {
                block: BasicBlockId::from_index(0),
                location: InitializationLocation::Statement(2),
                local: source,
            }]
        );
    }
}
