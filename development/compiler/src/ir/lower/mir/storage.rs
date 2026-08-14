//! Projection of logical MIR locals onto machine-IR local slots.
//!
//! MIR locals always have real logical storage. This late projection may omit
//! a local only when the machine-IR conversion consumes its sole definition
//! and use as one expression.

use crate::mir::{
    BasicBlockId, Body, CallContinuation, LocalId, LocalOrigin, LocalStorage, Operand, Rvalue,
    ValueRepresentation,
};

pub(super) fn machine_local_index(body: &Body, local: LocalId) -> usize {
    body.locals[..local.index()]
        .iter()
        .enumerate()
        .map(|(index, local)| machine_word_width(body, LocalId::from_index(index), local))
        .sum()
}

pub(super) fn machine_local_count(body: &Body) -> usize {
    body.locals
        .iter()
        .enumerate()
        .map(|(index, local)| machine_word_width(body, LocalId::from_index(index), local))
        .sum()
}

fn machine_word_width(body: &Body, id: LocalId, local: &crate::mir::Local) -> usize {
    if local.storage != LocalStorage::Local
        || is_inlined_loop_condition(body, id)
        || is_inlined_borrow_temporary(body, id)
    {
        return 0;
    }
    match local.representation {
        ValueRepresentation::Aggregate => 0,
        ValueRepresentation::Error => 4,
        ValueRepresentation::Scalar(_) | ValueRepresentation::Borrow => 1,
    }
}

pub(super) fn is_inlined_borrow_temporary(body: &Body, local: LocalId) -> bool {
    let Some(declaration) = body.locals.get(local.index()) else {
        return false;
    };
    declaration.storage == LocalStorage::Local
        && declaration.representation == ValueRepresentation::Borrow
        && matches!(declaration.origin, LocalOrigin::Temporary(_))
        && local_definition_count(body, local) == 1
        && local_use_count(body, local) == 1
}

pub(super) fn inlined_borrow_source(body: &Body, local: LocalId) -> Option<crate::mir::Place> {
    if !is_inlined_borrow_temporary(body, local) {
        return None;
    }
    body.loans
        .iter()
        .find(|loan| loan.destination == local)
        .map(|loan| loan.source)
}

pub(super) fn inlined_loop_condition_local(
    body: &Body,
    condition_block: BasicBlockId,
) -> Option<LocalId> {
    let block = body.blocks.get(condition_block.index())?;
    let crate::mir::Terminator::Switch { condition, .. } = &block.terminator else {
        return None;
    };
    let Operand::Copy(condition) = condition else {
        return None;
    };
    let Some(crate::mir::Statement::Assign {
        destination,
        value: Rvalue::Compare { .. },
        ..
    }) = block.statements.last()
    else {
        return None;
    };
    (destination == condition
        && local_definition_count(body, destination.local) == 1
        && local_use_count(body, destination.local) == 1)
        .then_some(destination.local)
}

fn is_inlined_loop_condition(body: &Body, local: LocalId) -> bool {
    body.loop_regions.iter().any(|region| {
        inlined_loop_condition_local(body, region.condition)
            .is_some_and(|candidate| candidate == local)
    })
}

fn local_definition_count(body: &Body, local: LocalId) -> usize {
    body.blocks
        .iter()
        .map(|block| {
            let statements = block
                .statements
                .iter()
                .filter(|statement| match statement {
                    crate::mir::Statement::BeginAggregate { .. }
                    | crate::mir::Statement::FinishAggregate { .. } => false,
                    crate::mir::Statement::Assign { destination, .. } => destination.local == local,
                    crate::mir::Statement::BeginLoan { loan, .. } => body
                        .loans
                        .get(loan.index())
                        .is_some_and(|loan| loan.destination == local),
                    crate::mir::Statement::EndLoan { .. } => false,
                    crate::mir::Statement::EnterRegion { region, .. } => body
                        .allocation_regions
                        .get(region.index())
                        .is_some_and(|region| {
                            [
                                region.allocator,
                                region.state,
                                region.parent_state,
                                region.parent_kind,
                            ]
                            .contains(&local)
                        }),
                    crate::mir::Statement::ExitRegion { .. } => false,
                })
                .count();
            let terminator = match &block.terminator {
                crate::mir::Terminator::Call {
                    continuation:
                        CallContinuation::Return { destination, .. }
                        | CallContinuation::Outcome { destination, .. },
                    ..
                } if destination.local == local => 1,
                _ => 0,
            };
            statements + terminator
        })
        .sum()
}

fn local_use_count(body: &Body, local: LocalId) -> usize {
    body.blocks
        .iter()
        .map(|block| {
            let statements = block
                .statements
                .iter()
                .map(|statement| match statement {
                    crate::mir::Statement::BeginAggregate { .. }
                    | crate::mir::Statement::FinishAggregate { .. } => 0,
                    crate::mir::Statement::Assign { value, .. } => rvalue_use_count(value, local),
                    crate::mir::Statement::BeginLoan { loan, .. } => body
                        .loans
                        .get(loan.index())
                        .is_some_and(|loan| loan.source.local == local)
                        as usize,
                    crate::mir::Statement::EndLoan { .. } => 0,
                    crate::mir::Statement::EnterRegion { region, .. } => body
                        .allocation_regions
                        .get(region.index())
                        .is_some_and(|region| region.parent.local == local)
                        as usize,
                    crate::mir::Statement::ExitRegion { region } => body
                        .allocation_regions
                        .get(region.index())
                        .map(|region| {
                            [region.state, region.parent_state, region.parent_kind]
                                .into_iter()
                                .filter(|candidate| *candidate == local)
                                .count()
                        })
                        .unwrap_or(0),
                })
                .sum::<usize>();
            let terminator = match &block.terminator {
                crate::mir::Terminator::Switch { condition, .. } => {
                    operand_use_count(condition, local)
                }
                crate::mir::Terminator::Call { arguments, .. } => arguments
                    .iter()
                    .map(|argument| operand_use_count(&argument.operand, local))
                    .sum(),
                crate::mir::Terminator::Goto { .. }
                | crate::mir::Terminator::Drop { .. }
                | crate::mir::Terminator::Trap
                | crate::mir::Terminator::PropagateFailure
                | crate::mir::Terminator::Return => 0,
            };
            statements + terminator
        })
        .sum()
}

fn rvalue_use_count(value: &Rvalue, local: LocalId) -> usize {
    match value {
        Rvalue::Use(operand) => operand_use_count(operand, local),
        Rvalue::Variant { leaves, .. } => leaves
            .iter()
            .map(|leaf| operand_use_count(&leaf.operand, local))
            .sum(),
        Rvalue::Unary { operand, .. } | Rvalue::Cast { operand, .. } => {
            operand_use_count(operand, local)
        }
        Rvalue::Binary { left, right, .. } | Rvalue::Compare { left, right, .. } => {
            operand_use_count(left, local) + operand_use_count(right, local)
        }
    }
}

fn operand_use_count(operand: &Operand, local: LocalId) -> usize {
    usize::from(
        matches!(operand, Operand::Copy(place) | Operand::Move(place) if place.local == local),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, Constant, Local, LocalOrigin, LoopRegion, Place, ScalarType, Statement,
        Terminator,
    };
    use crate::semantic::{BodyId, ExprId, TyId};
    use crate::source::{ByteSpan, SourceId};

    fn body_with_condition(extra_use: bool) -> Body {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let ty = TyId::from_index(0);
        let root_scope = crate::mir::ScopeId::from_index(0);
        let condition = LocalId::from_index(1);
        let mut statements = vec![Statement::Assign {
            destination: Place::local(condition),
            value: Rvalue::Compare {
                operator: crate::mir::ComparisonOperator::Equal,
                left: Operand::Constant(Constant {
                    ty,
                    scalar: ScalarType::I32,
                    value: 1,
                }),
                right: Operand::Constant(Constant {
                    ty,
                    scalar: ScalarType::I32,
                    value: 1,
                }),
                operand_ty: ty,
                operand_scalar: ScalarType::I32,
                result_ty: ty,
            },
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
        }];
        if extra_use {
            statements.insert(
                0,
                Statement::Assign {
                    destination: Place::local(LocalId::from_index(0)),
                    value: Rvalue::Use(Operand::Copy(Place::local(condition))),
                    origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
                },
            );
        }
        Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: crate::mir::ReturnMode::Plain,
            root_scope,
            scopes: vec![crate::mir::Scope::root(span)],
            locals: vec![
                Local::scalar(
                    ty,
                    ScalarType::Bool,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    root_scope,
                ),
                Local::scalar(
                    ty,
                    ScalarType::Bool,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(ExprId::from_index(0)),
                    root_scope,
                ),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    scope: root_scope,
                    statements,
                    terminator: Terminator::Switch {
                        condition: Operand::Copy(Place::local(condition)),
                        then_target: BasicBlockId::from_index(1),
                        else_target: BasicBlockId::from_index(2),
                    },
                },
                BasicBlock {
                    scope: root_scope,
                    statements: Vec::new(),
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(0),
                    },
                },
                BasicBlock {
                    scope: root_scope,
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ],
            loop_regions: vec![LoopRegion {
                header: BasicBlockId::from_index(0),
                condition: BasicBlockId::from_index(0),
                body: BasicBlockId::from_index(1),
                continue_target: BasicBlockId::from_index(0),
                exit: BasicBlockId::from_index(2),
            }],
            allocation_regions: Vec::new(),
            loans: Vec::new(),
            projections: Vec::new(),
            drop_plans: Vec::new(),
        }
    }

    #[test]
    fn omits_a_single_use_inlined_loop_condition() {
        let body = body_with_condition(false);
        assert_eq!(
            inlined_loop_condition_local(&body, BasicBlockId::from_index(0)),
            Some(LocalId::from_index(1))
        );
    }

    #[test]
    fn retains_a_condition_local_with_another_use() {
        let body = body_with_condition(true);
        assert_eq!(
            inlined_loop_condition_local(&body, BasicBlockId::from_index(0)),
            None
        );
    }
}
