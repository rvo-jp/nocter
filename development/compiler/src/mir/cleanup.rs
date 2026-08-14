//! Materialization of lexical cleanup edges in checked MIR.
//!
//! This pass consumes definite-initialization facts and the retained scope
//! tree. It does not inspect source blocks or rediscover exits from syntax.

use super::initialization::InitializationAnalysis;
use super::locals::{OwnershipKind, ValueRepresentation};
use super::model::BasicBlock;
use super::{
    AllocationOverrideId, BasicBlockId, Body, LoanId, LocalId, Place, RegionId, ScopeId, Statement,
    Terminator,
};

#[derive(Debug, Clone, Copy)]
enum CleanupAction {
    EndLoan(LoanId),
    Drop(Place),
    Region(RegionId),
    Override(AllocationOverrideId),
}

pub(super) fn materialize(body: &mut Body) {
    let analysis = super::initialization::analyze(body);
    let loans = super::loans::analyze(body);
    let original_block_count = body.blocks.len();
    for index in 0..original_block_count {
        let block_id = BasicBlockId::from_index(index);
        let source_scope = body.blocks[index].scope;
        let terminator = body.blocks[index].terminator.clone();
        body.blocks[index].terminator = match terminator {
            Terminator::Goto { target } => Terminator::Goto {
                target: cleanup_edge(body, &analysis, &loans, block_id, source_scope, target),
            },
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => Terminator::Switch {
                condition,
                then_target: cleanup_edge(
                    body,
                    &analysis,
                    &loans,
                    block_id,
                    source_scope,
                    then_target,
                ),
                else_target: cleanup_edge(
                    body,
                    &analysis,
                    &loans,
                    block_id,
                    source_scope,
                    else_target,
                ),
            },
            Terminator::Call {
                origin,
                callee,
                arguments,
                continuation,
            } => Terminator::Call {
                origin,
                callee,
                arguments,
                continuation: match continuation {
                    super::CallContinuation::Continue { target } => {
                        super::CallContinuation::Continue {
                            target: cleanup_edge(
                                body,
                                &analysis,
                                &loans,
                                block_id,
                                source_scope,
                                target,
                            ),
                        }
                    }
                    super::CallContinuation::Return {
                        destination,
                        target,
                    } => super::CallContinuation::Return {
                        destination,
                        target: cleanup_edge(
                            body,
                            &analysis,
                            &loans,
                            block_id,
                            source_scope,
                            target,
                        ),
                    },
                    super::CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                        failure_payload,
                    } => super::CallContinuation::Outcome {
                        destination,
                        success: cleanup_edge(
                            body,
                            &analysis,
                            &loans,
                            block_id,
                            source_scope,
                            success,
                        ),
                        failure: cleanup_edge(
                            body,
                            &analysis,
                            &loans,
                            block_id,
                            source_scope,
                            failure,
                        ),
                        failure_payload,
                    },
                    super::CallContinuation::OutcomeEffect {
                        success,
                        failure,
                        failure_payload,
                    } => super::CallContinuation::OutcomeEffect {
                        success: cleanup_edge(
                            body,
                            &analysis,
                            &loans,
                            block_id,
                            source_scope,
                            success,
                        ),
                        failure: cleanup_edge(
                            body,
                            &analysis,
                            &loans,
                            block_id,
                            source_scope,
                            failure,
                        ),
                        failure_payload,
                    },
                    super::CallContinuation::Never => super::CallContinuation::Never,
                },
            },
            Terminator::InspectOutcome {
                origin,
                source,
                layer,
                destination,
                success,
                failure,
                failure_payload,
            } => Terminator::InspectOutcome {
                origin,
                source,
                layer,
                destination,
                success: cleanup_edge(body, &analysis, &loans, block_id, source_scope, success),
                failure: cleanup_edge(body, &analysis, &loans, block_id, source_scope, failure),
                failure_payload,
            },
            Terminator::Return => cleanup_exit(
                body,
                &analysis,
                &loans,
                block_id,
                source_scope,
                Terminator::Return,
            ),
            Terminator::PropagateFailure => cleanup_exit(
                body,
                &analysis,
                &loans,
                block_id,
                source_scope,
                Terminator::PropagateFailure,
            ),
            Terminator::ReturnOutcome { source } => cleanup_exit(
                body,
                &analysis,
                &loans,
                block_id,
                source_scope,
                Terminator::ReturnOutcome { source },
            ),
            Terminator::ReturnFailure { code, message } => cleanup_exit(
                body,
                &analysis,
                &loans,
                block_id,
                source_scope,
                Terminator::ReturnFailure { code, message },
            ),
            Terminator::Drop {
                place,
                plan,
                target,
            } => Terminator::Drop {
                place,
                plan,
                target,
            },
            Terminator::Trap => Terminator::Trap,
        };
    }
}

fn cleanup_edge(
    body: &mut Body,
    analysis: &InitializationAnalysis,
    loans: &super::loans::LoanAnalysis,
    from: BasicBlockId,
    source_scope: ScopeId,
    target: BasicBlockId,
) -> BasicBlockId {
    let Some(target_scope) = body.blocks.get(target.index()).map(|block| block.scope) else {
        return target;
    };
    let Some(exited) = super::scopes::exited_scopes(&body.scopes, source_scope, target_scope)
    else {
        return target;
    };
    let actions = cleanup_actions(
        body,
        &exited,
        |place| analysis.initialized_on_edge(body, from, target, place),
        |loan| loans.definitely_active_at_exit(from, loan),
    );
    prepend_cleanup_chain(body, actions, target)
}

fn cleanup_exit(
    body: &mut Body,
    analysis: &InitializationAnalysis,
    loans: &super::loans::LoanAnalysis,
    block: BasicBlockId,
    source_scope: ScopeId,
    terminal: Terminator,
) -> Terminator {
    let exited = scope_ancestors(body, source_scope);
    let actions = cleanup_actions(
        body,
        &exited,
        |place| analysis.initialized_at_exit(body, block, place),
        |loan| loans.definitely_active_at_exit(block, loan),
    );
    if actions.is_empty() {
        return terminal;
    }
    let terminal_block = BasicBlockId::from_index(body.blocks.len());
    body.blocks.push(BasicBlock {
        scope: body.root_scope,
        statements: Vec::new(),
        terminator: terminal,
    });
    Terminator::Goto {
        target: prepend_cleanup_chain(body, actions, terminal_block),
    }
}

fn cleanup_actions(
    body: &Body,
    exited: &[ScopeId],
    initialized: impl Fn(Place) -> bool,
    active: impl Fn(LoanId) -> bool,
) -> Vec<CleanupAction> {
    let mut actions = Vec::new();
    for scope in exited {
        actions.extend(
            body.loans
                .iter()
                .rev()
                .filter(|loan| loan.scope == *scope && active(loan.id))
                .map(|loan| CleanupAction::EndLoan(loan.id)),
        );
        actions.extend(
            cleanup_places(body, std::slice::from_ref(scope), &initialized)
                .into_iter()
                .map(CleanupAction::Drop),
        );
        actions.extend(
            body.allocation_overrides
                .iter()
                .rev()
                .filter(|override_| override_.scope == *scope)
                .map(|override_| CleanupAction::Override(override_.id)),
        );
        actions.extend(
            body.allocation_regions
                .iter()
                .rev()
                .filter(|region| region.scope == *scope)
                .map(|region| CleanupAction::Region(region.id)),
        );
    }
    actions
}

fn cleanup_places(
    body: &Body,
    exited: &[ScopeId],
    initialized: impl Fn(Place) -> bool,
) -> Vec<Place> {
    let mut places = Vec::new();
    for scope in exited {
        for (index, local) in body.locals.iter().enumerate().rev() {
            let id = LocalId::from_index(index);
            if id != body.return_local
                && local.scope == *scope
                && local.representation == ValueRepresentation::Aggregate
                && local.ownership == OwnershipKind::Move
            {
                let root = Place::local(id);
                if initialized(root) {
                    places.push(root);
                    continue;
                }
                for projection in body.projections.iter().rev().filter(|projection| {
                    projection.base == id
                        && projection.representation == ValueRepresentation::Aggregate
                        && projection.ownership == OwnershipKind::Move
                }) {
                    let place = Place::projected(id, projection.id);
                    if initialized(place)
                        && projection
                            .parent
                            .is_none_or(|parent| !initialized(Place::projected(id, parent)))
                    {
                        places.push(place);
                    }
                }
            }
        }
    }
    places
}

fn prepend_cleanup_chain(
    body: &mut Body,
    actions: Vec<CleanupAction>,
    final_target: BasicBlockId,
) -> BasicBlockId {
    let mut target = final_target;
    for action in actions.into_iter().rev() {
        let block = BasicBlockId::from_index(body.blocks.len());
        let (scope, statements, terminator) = match action {
            CleanupAction::EndLoan(loan) => (
                body.root_scope,
                vec![Statement::EndLoan { loan }],
                Terminator::Goto { target },
            ),
            CleanupAction::Drop(place) => (
                body.locals[place.local.index()].scope,
                Vec::new(),
                Terminator::Drop {
                    place,
                    plan: place
                        .projection
                        .and_then(|projection| body.projections[projection.index()].drop_plan)
                        .or(body.locals[place.local.index()].drop_plan)
                        .expect("owned MIR local must carry a semantic drop plan"),
                    target,
                },
            ),
            CleanupAction::Region(region) => (
                body.root_scope,
                vec![Statement::ExitRegion { region }],
                Terminator::Goto { target },
            ),
            CleanupAction::Override(override_) => (
                body.root_scope,
                vec![Statement::ExitAllocationContext { override_ }],
                Terminator::Goto { target },
            ),
        };
        body.blocks.push(BasicBlock {
            scope,
            statements,
            terminator,
        });
        target = block;
    }
    target
}

fn scope_ancestors(body: &Body, start: ScopeId) -> Vec<ScopeId> {
    let mut result = Vec::new();
    let mut current = Some(start);
    while let Some(scope) = current {
        let Some(record) = body.scopes.get(scope.index()) else {
            break;
        };
        result.push(scope);
        current = record.parent;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        CallContinuation, Local, LocalOrigin, LocalStorage, Origin, OwnershipKind, ReturnMode,
        Rvalue, ScalarType, Scope, Statement,
    };
    use crate::semantic::{BodyId, DefId, ExprId, TyId};
    use crate::source::{ByteSpan, SourceId};

    #[test]
    fn materializes_owned_local_cleanup_before_return() {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let scope = ScopeId::from_index(0);
        let scalar_ty = TyId::from_index(0);
        let aggregate_ty = TyId::from_index(1);
        let owned = LocalId::from_index(1);
        let mut body = Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            root_scope: scope,
            scopes: vec![Scope::root(span)],
            locals: vec![
                Local::scalar(
                    scalar_ty,
                    ScalarType::I32,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    scope,
                ),
                Local::aggregate(
                    aggregate_ty,
                    OwnershipKind::Move,
                    LocalStorage::Local,
                    LocalOrigin::Desugared(span),
                    scope,
                )
                .with_drop_plan(crate::mir::DropPlanId::from_index(0)),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    scope,
                    statements: Vec::new(),
                    terminator: Terminator::Call {
                        origin: Origin::Expression(ExprId::from_index(0)),
                        callee: crate::mir::CallInstance::direct(DefId::from_index(0)),
                        arguments: Vec::new(),
                        continuation: CallContinuation::Return {
                            destination: Place::local(owned),
                            target: BasicBlockId::from_index(1),
                        },
                    },
                },
                BasicBlock {
                    scope,
                    statements: vec![Statement::Assign {
                        destination: Place::local(LocalId::from_index(0)),
                        value: Rvalue::Use(crate::mir::Operand::Constant(crate::mir::Constant {
                            ty: scalar_ty,
                            scalar: ScalarType::I32,
                            value: 0,
                        })),
                        origin: Origin::Expression(ExprId::from_index(1)),
                    }],
                    terminator: Terminator::Return,
                },
            ],
            loop_regions: Vec::new(),
            allocation_regions: Vec::new(),
            allocation_overrides: Vec::new(),
            loans: Vec::new(),
            projections: Vec::new(),
            drop_plans: vec![crate::mir::DropPlan::Direct {
                destructor: DefId::from_index(0),
            }],
        };

        assert!(crate::mir::validate(&body).is_err());
        materialize(&mut body);
        assert!(crate::mir::validate(&body).is_ok());
        assert!(body.blocks.iter().any(|block| {
            matches!(
                block.terminator,
                Terminator::Drop { place, .. } if place == Place::local(owned)
            )
        }));
    }

    #[test]
    fn materializes_remaining_owned_projection_after_partial_move() {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let scope = ScopeId::from_index(0);
        let scalar_ty = TyId::from_index(0);
        let aggregate_ty = TyId::from_index(1);
        let source = LocalId::from_index(1);
        let moved_field = LocalId::from_index(2);
        let first = crate::mir::ProjectionPathId::from_index(0);
        let second = crate::mir::ProjectionPathId::from_index(1);
        let mut body = Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            root_scope: scope,
            scopes: vec![Scope::root(span)],
            locals: vec![
                Local::scalar(
                    scalar_ty,
                    ScalarType::I32,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    scope,
                ),
                Local::aggregate(
                    aggregate_ty,
                    OwnershipKind::Move,
                    LocalStorage::Local,
                    LocalOrigin::Desugared(span),
                    scope,
                )
                .with_drop_plan(crate::mir::DropPlanId::from_index(0)),
                Local::aggregate(
                    aggregate_ty,
                    OwnershipKind::Move,
                    LocalStorage::Local,
                    LocalOrigin::Desugared(span),
                    scope,
                )
                .with_drop_plan(crate::mir::DropPlanId::from_index(0)),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    scope,
                    statements: Vec::new(),
                    terminator: Terminator::Call {
                        origin: Origin::Expression(ExprId::from_index(0)),
                        callee: crate::mir::CallInstance::direct(DefId::from_index(0)),
                        arguments: Vec::new(),
                        continuation: CallContinuation::Return {
                            destination: Place::local(source),
                            target: BasicBlockId::from_index(1),
                        },
                    },
                },
                BasicBlock {
                    scope,
                    statements: vec![
                        Statement::Assign {
                            destination: Place::local(moved_field),
                            value: Rvalue::Use(crate::mir::Operand::Move(Place::projected(
                                source, first,
                            ))),
                            origin: Origin::Expression(ExprId::from_index(1)),
                        },
                        Statement::Assign {
                            destination: Place::local(LocalId::from_index(0)),
                            value: Rvalue::Use(crate::mir::Operand::Constant(
                                crate::mir::Constant {
                                    ty: scalar_ty,
                                    scalar: ScalarType::I32,
                                    value: 0,
                                },
                            )),
                            origin: Origin::Expression(ExprId::from_index(2)),
                        },
                    ],
                    terminator: Terminator::Return,
                },
            ],
            loop_regions: Vec::new(),
            allocation_regions: Vec::new(),
            allocation_overrides: Vec::new(),
            loans: Vec::new(),
            projections: vec![
                crate::mir::ProjectionPath {
                    id: first,
                    base: source,
                    parent: None,
                    element: crate::mir::ProjectionElement::Field { offset: 0 },
                    ty: aggregate_ty,
                    representation: ValueRepresentation::Aggregate,
                    ownership: OwnershipKind::Move,
                    drop_plan: Some(crate::mir::DropPlanId::from_index(0)),
                },
                crate::mir::ProjectionPath {
                    id: second,
                    base: source,
                    parent: None,
                    element: crate::mir::ProjectionElement::Field { offset: 8 },
                    ty: aggregate_ty,
                    representation: ValueRepresentation::Aggregate,
                    ownership: OwnershipKind::Move,
                    drop_plan: Some(crate::mir::DropPlanId::from_index(0)),
                },
            ],
            drop_plans: vec![crate::mir::DropPlan::Direct {
                destructor: DefId::from_index(0),
            }],
        };

        assert!(crate::mir::validate(&body).is_err());
        materialize(&mut body);
        assert!(crate::mir::validate(&body).is_ok());
        assert!(body.blocks.iter().any(|block| {
            matches!(
                block.terminator,
                Terminator::Drop { place, .. }
                    if place == Place::projected(source, second)
            )
        }));
        assert!(!body.blocks.iter().any(|block| {
            matches!(
                block.terminator,
                Terminator::Drop { place, .. }
                    if place == Place::local(source)
            )
        }));
    }
}
