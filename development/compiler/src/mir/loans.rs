//! Path-sensitive validation of MIR loans.
//!
//! Borrow types describe values; loans describe one concrete borrow's live
//! interval. Explicit begin/end statements make ownership conflicts a CFG
//! property instead of something reconstructed from AST nesting.

use super::dataflow::LoanSet;
use super::{
    BasicBlockId, Body, BorrowKind, CallContinuation, LoanId, Operand, OwnershipKind, Place,
    Rvalue, Statement, Terminator,
};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LoanErrorKind {
    InvalidIdentity,
    InvalidSource,
    InvalidDestination,
    InvalidScope,
    AlreadyActive,
    ConflictingBorrow,
    EndOfInactive,
    MutateWhileBorrowed,
    MoveWhileBorrowed,
    LiveAtExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LoanError {
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) loan: LoanId,
    pub(crate) kind: LoanErrorKind,
}

#[derive(Debug, Clone)]
struct LoanState {
    may_active: LoanSet,
    must_active: LoanSet,
}

pub(super) fn validate(body: &Body) -> Vec<LoanError> {
    let mut errors = HashSet::new();
    validate_declarations(body, &mut errors);
    if body.blocks.get(body.entry.index()).is_none() {
        return sorted(errors);
    }

    let initial = LoanSet::new(body.loans.len());
    let mut entries = vec![None; body.blocks.len()];
    entries[body.entry.index()] = Some(LoanState {
        may_active: initial.clone(),
        must_active: initial,
    });
    let mut queue = VecDeque::from([body.entry]);

    while let Some(block_id) = queue.pop_front() {
        let Some(block) = body.blocks.get(block_id.index()) else {
            continue;
        };
        let Some(mut state) = entries[block_id.index()].clone() else {
            continue;
        };

        for statement in &block.statements {
            match statement {
                Statement::Assign {
                    destination, value, ..
                } => {
                    reject_mutation(body, block_id, *destination, &state, &mut errors);
                    reject_rvalue_moves(body, block_id, value, &state, &mut errors);
                }
                Statement::BeginLoan { loan, .. } => {
                    let Some(declaration) = body.loans.get(loan.index()) else {
                        errors.insert(LoanError {
                            block: Some(block_id),
                            loan: *loan,
                            kind: LoanErrorKind::InvalidIdentity,
                        });
                        continue;
                    };
                    if state.may_active.contains(*loan) {
                        errors.insert(LoanError {
                            block: Some(block_id),
                            loan: *loan,
                            kind: LoanErrorKind::AlreadyActive,
                        });
                    }
                    for (index, other) in body.loans.iter().enumerate() {
                        let other_id = LoanId::from_index(index);
                        if state.may_active.contains(other_id)
                            && super::places::overlap(body, other.source, declaration.source)
                            && (other.kind == BorrowKind::Readwrite
                                || declaration.kind == BorrowKind::Readwrite)
                        {
                            errors.insert(LoanError {
                                block: Some(block_id),
                                loan: *loan,
                                kind: LoanErrorKind::ConflictingBorrow,
                            });
                        }
                    }
                    state.may_active.insert(*loan);
                    state.must_active.insert(*loan);
                }
                Statement::EndLoan { loan } => {
                    if !state.must_active.contains(*loan) {
                        errors.insert(LoanError {
                            block: Some(block_id),
                            loan: *loan,
                            kind: LoanErrorKind::EndOfInactive,
                        });
                    }
                    state.may_active.remove(*loan);
                    state.must_active.remove(*loan);
                }
            }
        }

        match &block.terminator {
            Terminator::Goto { target } => merge_entry(&mut entries, &mut queue, *target, state),
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                reject_operand_move(body, block_id, condition, &state, &mut errors);
                merge_entry(&mut entries, &mut queue, *then_target, state.clone());
                merge_entry(&mut entries, &mut queue, *else_target, state);
            }
            Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                for argument in arguments {
                    reject_operand_move(body, block_id, &argument.operand, &state, &mut errors);
                }
                match continuation {
                    CallContinuation::Return { target, .. } => {
                        merge_entry(&mut entries, &mut queue, *target, state)
                    }
                    CallContinuation::Outcome {
                        success, failure, ..
                    } => {
                        merge_entry(&mut entries, &mut queue, *success, state.clone());
                        merge_entry(&mut entries, &mut queue, *failure, state);
                    }
                    CallContinuation::Never => {
                        reject_live_at_exit(body, block_id, &state, &mut errors)
                    }
                }
            }
            Terminator::Drop { place, target } => {
                reject_move(body, block_id, *place, &state, &mut errors);
                merge_entry(&mut entries, &mut queue, *target, state);
            }
            Terminator::Trap | Terminator::PropagateFailure | Terminator::Return => {
                reject_live_at_exit(body, block_id, &state, &mut errors);
            }
        }
    }

    sorted(errors)
}

fn validate_declarations(body: &Body, errors: &mut HashSet<LoanError>) {
    for (index, loan) in body.loans.iter().enumerate() {
        let expected = LoanId::from_index(index);
        if loan.id != expected {
            errors.insert(LoanError {
                block: None,
                loan: expected,
                kind: LoanErrorKind::InvalidIdentity,
            });
        }
        if body.locals.get(loan.source.local.index()).is_none()
            || loan.source.projection.is_some_and(|projection| {
                body.projections
                    .get(projection.index())
                    .is_none_or(|path| path.base != loan.source.local)
            })
        {
            errors.insert(LoanError {
                block: None,
                loan: expected,
                kind: LoanErrorKind::InvalidSource,
            });
        }
        let destination_valid = body
            .locals
            .get(loan.destination.index())
            .is_some_and(|local| {
                matches!(
                    (loan.kind, local.ownership),
                    (
                        BorrowKind::Readonly,
                        OwnershipKind::Borrowed { readwrite: false }
                    ) | (
                        BorrowKind::Readwrite,
                        OwnershipKind::Borrowed { readwrite: true }
                    )
                )
            });
        if !destination_valid {
            errors.insert(LoanError {
                block: None,
                loan: expected,
                kind: LoanErrorKind::InvalidDestination,
            });
        }
        if body.scopes.get(loan.scope.index()).is_none() {
            errors.insert(LoanError {
                block: None,
                loan: expected,
                kind: LoanErrorKind::InvalidScope,
            });
        }
    }
}

fn reject_rvalue_moves(
    body: &Body,
    block: BasicBlockId,
    value: &Rvalue,
    state: &LoanState,
    errors: &mut HashSet<LoanError>,
) {
    match value {
        Rvalue::Use(operand) => reject_operand_move(body, block, operand, state, errors),
        Rvalue::Binary { left, right, .. } | Rvalue::Compare { left, right, .. } => {
            reject_operand_move(body, block, left, state, errors);
            reject_operand_move(body, block, right, state, errors);
        }
    }
}

fn reject_operand_move(
    body: &Body,
    block: BasicBlockId,
    operand: &Operand,
    state: &LoanState,
    errors: &mut HashSet<LoanError>,
) {
    if let Operand::Move(place) = operand {
        reject_move(body, block, *place, state, errors);
    }
}

fn reject_mutation(
    body: &Body,
    block: BasicBlockId,
    place: Place,
    state: &LoanState,
    errors: &mut HashSet<LoanError>,
) {
    for (index, loan) in body.loans.iter().enumerate() {
        let id = LoanId::from_index(index);
        if state.may_active.contains(id) && super::places::overlap(body, loan.source, place) {
            errors.insert(LoanError {
                block: Some(block),
                loan: id,
                kind: LoanErrorKind::MutateWhileBorrowed,
            });
        }
    }
}

fn reject_move(
    body: &Body,
    block: BasicBlockId,
    place: Place,
    state: &LoanState,
    errors: &mut HashSet<LoanError>,
) {
    for (index, loan) in body.loans.iter().enumerate() {
        let id = LoanId::from_index(index);
        if state.may_active.contains(id) && super::places::overlap(body, loan.source, place) {
            errors.insert(LoanError {
                block: Some(block),
                loan: id,
                kind: LoanErrorKind::MoveWhileBorrowed,
            });
        }
    }
}

fn reject_live_at_exit(
    body: &Body,
    block: BasicBlockId,
    state: &LoanState,
    errors: &mut HashSet<LoanError>,
) {
    for index in 0..body.loans.len() {
        let id = LoanId::from_index(index);
        if state.may_active.contains(id) {
            errors.insert(LoanError {
                block: Some(block),
                loan: id,
                kind: LoanErrorKind::LiveAtExit,
            });
        }
    }
}

fn merge_entry(
    entries: &mut [Option<LoanState>],
    queue: &mut VecDeque<BasicBlockId>,
    target: BasicBlockId,
    incoming: LoanState,
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
            let may_changed = existing.may_active.union_with(&incoming.may_active);
            let must_changed = existing.must_active.intersect_with(&incoming.must_active);
            may_changed || must_changed
        }
    };
    if changed {
        queue.push_back(target);
    }
}

fn sorted(errors: HashSet<LoanError>) -> Vec<LoanError> {
    let mut errors = errors.into_iter().collect::<Vec<_>>();
    errors.sort_by_key(|error| {
        (
            error.block.map_or(usize::MAX, BasicBlockId::index),
            error.loan.index(),
            error_kind_order(error.kind),
        )
    });
    errors
}

fn error_kind_order(kind: LoanErrorKind) -> usize {
    match kind {
        LoanErrorKind::InvalidIdentity => 0,
        LoanErrorKind::InvalidSource => 1,
        LoanErrorKind::InvalidDestination => 2,
        LoanErrorKind::InvalidScope => 3,
        LoanErrorKind::AlreadyActive => 4,
        LoanErrorKind::ConflictingBorrow => 5,
        LoanErrorKind::EndOfInactive => 6,
        LoanErrorKind::MutateWhileBorrowed => 7,
        LoanErrorKind::MoveWhileBorrowed => 8,
        LoanErrorKind::LiveAtExit => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, Loan, Local, LocalId, LocalOrigin, LocalStorage, Origin, Place, ReturnMode,
        ScalarType, Scope, ScopeId,
    };
    use crate::semantic::{BodyId, ExprId, TyId};
    use crate::source::{ByteSpan, SourceId};

    fn span() -> ByteSpan {
        ByteSpan::new(SourceId::new(0), 0, 1)
    }

    fn block(statements: Vec<Statement>, terminator: Terminator) -> BasicBlock {
        BasicBlock {
            scope: ScopeId::from_index(0),
            statements,
            terminator,
        }
    }

    fn loan_statement(begin: bool, loan: usize) -> Statement {
        if begin {
            Statement::BeginLoan {
                loan: LoanId::from_index(loan),
                origin: Origin::Expression(ExprId::from_index(loan)),
            }
        } else {
            Statement::EndLoan {
                loan: LoanId::from_index(loan),
            }
        }
    }

    fn body(kinds: &[BorrowKind], blocks: Vec<BasicBlock>) -> Body {
        let span = span();
        let scope = ScopeId::from_index(0);
        let source_ty = TyId::from_index(0);
        let borrow_ty = TyId::from_index(1);
        let mut locals = vec![
            Local::scalar(
                source_ty,
                ScalarType::I32,
                LocalStorage::Return,
                LocalOrigin::Return,
                scope,
            ),
            Local::aggregate(
                source_ty,
                OwnershipKind::Move,
                LocalStorage::Local,
                LocalOrigin::Desugared(span),
                scope,
            ),
        ];
        let mut loans = Vec::new();
        for (index, kind) in kinds.iter().copied().enumerate() {
            let destination = LocalId::from_index(locals.len());
            locals.push(Local::borrow(
                borrow_ty,
                kind == BorrowKind::Readwrite,
                LocalStorage::Local,
                LocalOrigin::Desugared(span),
                scope,
            ));
            loans.push(Loan {
                id: LoanId::from_index(index),
                source: Place::local(LocalId::from_index(1)),
                destination,
                kind,
                scope,
            });
        }
        Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            root_scope: scope,
            scopes: vec![Scope::root(span)],
            locals,
            entry: BasicBlockId::from_index(0),
            blocks,
            loop_regions: Vec::new(),
            loans,
            projections: Vec::new(),
        }
    }

    #[test]
    fn ended_readonly_loan_is_valid() {
        let body = body(
            &[BorrowKind::Readonly],
            vec![block(
                vec![loan_statement(true, 0), loan_statement(false, 0)],
                Terminator::Return,
            )],
        );

        assert!(validate(&body).is_empty());
    }

    #[test]
    fn readwrite_loan_conflicts_with_live_readonly_loan() {
        let body = body(
            &[BorrowKind::Readonly, BorrowKind::Readwrite],
            vec![block(
                vec![
                    loan_statement(true, 0),
                    loan_statement(true, 1),
                    loan_statement(false, 1),
                    loan_statement(false, 0),
                ],
                Terminator::Return,
            )],
        );

        assert!(validate(&body).iter().any(|error| {
            error.loan == LoanId::from_index(1) && error.kind == LoanErrorKind::ConflictingBorrow
        }));
    }

    #[test]
    fn readwrite_loans_to_disjoint_fields_do_not_conflict() {
        let mut body = body(
            &[BorrowKind::Readwrite, BorrowKind::Readwrite],
            vec![block(
                vec![
                    loan_statement(true, 0),
                    loan_statement(true, 1),
                    loan_statement(false, 1),
                    loan_statement(false, 0),
                ],
                Terminator::Return,
            )],
        );
        let source = LocalId::from_index(1);
        body.projections = [0, 8]
            .into_iter()
            .enumerate()
            .map(|(index, offset)| crate::mir::ProjectionPath {
                id: crate::mir::ProjectionPathId::from_index(index),
                base: source,
                parent: None,
                element: crate::mir::ProjectionElement::Field { offset },
                ty: TyId::from_index(0),
                representation: crate::mir::ValueRepresentation::Aggregate,
                ownership: OwnershipKind::Move,
            })
            .collect();
        body.loans[0].source =
            Place::projected(source, crate::mir::ProjectionPathId::from_index(0));
        body.loans[1].source =
            Place::projected(source, crate::mir::ProjectionPathId::from_index(1));

        assert!(validate(&body).is_empty());
    }

    #[test]
    fn moving_borrowed_source_is_rejected() {
        let body = body(
            &[BorrowKind::Readonly],
            vec![block(
                vec![
                    loan_statement(true, 0),
                    Statement::Assign {
                        destination: Place::local(LocalId::from_index(0)),
                        value: Rvalue::Use(Operand::Move(Place::local(LocalId::from_index(1)))),
                        origin: Origin::Expression(ExprId::from_index(1)),
                    },
                    loan_statement(false, 0),
                ],
                Terminator::Return,
            )],
        );

        assert!(validate(&body).iter().any(|error| {
            error.loan == LoanId::from_index(0) && error.kind == LoanErrorKind::MoveWhileBorrowed
        }));
    }

    #[test]
    fn branch_local_end_makes_join_end_invalid() {
        let body = body(
            &[BorrowKind::Readonly],
            vec![
                block(
                    vec![loan_statement(true, 0)],
                    Terminator::Switch {
                        condition: Operand::Constant(crate::mir::Constant {
                            ty: TyId::from_index(0),
                            scalar: ScalarType::Bool,
                            value: 1,
                        }),
                        then_target: BasicBlockId::from_index(1),
                        else_target: BasicBlockId::from_index(2),
                    },
                ),
                block(
                    vec![loan_statement(false, 0)],
                    Terminator::Goto {
                        target: BasicBlockId::from_index(3),
                    },
                ),
                block(
                    Vec::new(),
                    Terminator::Goto {
                        target: BasicBlockId::from_index(3),
                    },
                ),
                block(vec![loan_statement(false, 0)], Terminator::Return),
            ],
        );

        assert!(validate(&body).iter().any(|error| {
            error.block == Some(BasicBlockId::from_index(3))
                && error.kind == LoanErrorKind::EndOfInactive
        }));
    }
}
