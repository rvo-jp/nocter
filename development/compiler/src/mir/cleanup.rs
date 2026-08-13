//! Materialization of lexical cleanup edges in checked MIR.
//!
//! This pass consumes definite-initialization facts and the retained scope
//! tree. It does not inspect source blocks or rediscover exits from syntax.

use super::initialization::InitializationAnalysis;
use super::locals::{OwnershipKind, ValueRepresentation};
use super::model::BasicBlock;
use super::{BasicBlockId, Body, LocalId, Place, ScopeId, Terminator};

#[allow(
    dead_code,
    reason = "owned aggregate route invokes cleanup materialization after its construction checkpoint"
)]
pub(super) fn materialize(body: &mut Body) {
    let analysis = super::initialization::analyze(body);
    let original_block_count = body.blocks.len();
    for index in 0..original_block_count {
        let block_id = BasicBlockId::from_index(index);
        let source_scope = body.blocks[index].scope;
        let terminator = body.blocks[index].terminator.clone();
        body.blocks[index].terminator = match terminator {
            Terminator::Goto { target } => Terminator::Goto {
                target: cleanup_edge(body, &analysis, block_id, source_scope, target),
            },
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => Terminator::Switch {
                condition,
                then_target: cleanup_edge(body, &analysis, block_id, source_scope, then_target),
                else_target: cleanup_edge(body, &analysis, block_id, source_scope, else_target),
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
                    super::CallContinuation::Return {
                        destination,
                        target,
                    } => super::CallContinuation::Return {
                        destination,
                        target: cleanup_edge(body, &analysis, block_id, source_scope, target),
                    },
                    super::CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => super::CallContinuation::Outcome {
                        destination,
                        success: cleanup_edge(body, &analysis, block_id, source_scope, success),
                        failure: cleanup_edge(body, &analysis, block_id, source_scope, failure),
                    },
                    super::CallContinuation::Never => super::CallContinuation::Never,
                },
            },
            Terminator::Return => {
                cleanup_exit(body, &analysis, block_id, source_scope, Terminator::Return)
            }
            Terminator::PropagateFailure => cleanup_exit(
                body,
                &analysis,
                block_id,
                source_scope,
                Terminator::PropagateFailure,
            ),
            Terminator::Drop { place, target } => Terminator::Drop { place, target },
            Terminator::Trap => Terminator::Trap,
        };
    }
}

fn cleanup_edge(
    body: &mut Body,
    analysis: &InitializationAnalysis,
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
    let locals = cleanup_locals(body, &exited, |local| {
        analysis.initialized_on_edge(from, target, local)
    });
    prepend_cleanup_chain(body, locals, target)
}

fn cleanup_exit(
    body: &mut Body,
    analysis: &InitializationAnalysis,
    block: BasicBlockId,
    source_scope: ScopeId,
    terminal: Terminator,
) -> Terminator {
    let exited = scope_ancestors(body, source_scope);
    let locals = cleanup_locals(body, &exited, |local| {
        analysis.initialized_at_exit(block, local)
    });
    if locals.is_empty() {
        return terminal;
    }
    let terminal_block = BasicBlockId::from_index(body.blocks.len());
    body.blocks.push(BasicBlock {
        scope: body.root_scope,
        statements: Vec::new(),
        terminator: terminal,
    });
    Terminator::Goto {
        target: prepend_cleanup_chain(body, locals, terminal_block),
    }
}

fn cleanup_locals(
    body: &Body,
    exited: &[ScopeId],
    initialized: impl Fn(LocalId) -> bool,
) -> Vec<LocalId> {
    let mut locals = Vec::new();
    for scope in exited {
        for (index, local) in body.locals.iter().enumerate().rev() {
            let id = LocalId::from_index(index);
            if id != body.return_local
                && local.scope == *scope
                && local.representation == ValueRepresentation::Aggregate
                && local.ownership == OwnershipKind::Move
                && initialized(id)
            {
                locals.push(id);
            }
        }
    }
    locals
}

fn prepend_cleanup_chain(
    body: &mut Body,
    locals: Vec<LocalId>,
    final_target: BasicBlockId,
) -> BasicBlockId {
    let mut target = final_target;
    for local in locals.into_iter().rev() {
        let block = BasicBlockId::from_index(body.blocks.len());
        body.blocks.push(BasicBlock {
            scope: body.locals[local.index()].scope,
            statements: Vec::new(),
            terminator: Terminator::Drop {
                place: Place::local(local),
                target,
            },
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
                ),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    scope,
                    statements: Vec::new(),
                    terminator: Terminator::Call {
                        origin: Origin::Expression(ExprId::from_index(0)),
                        callee: DefId::from_index(0),
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
            loans: Vec::new(),
            projections: Vec::new(),
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
}
