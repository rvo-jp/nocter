//! Construction-only state for splitting scalar expressions into MIR blocks.

use super::BuildError;
use crate::mir::ids::BasicBlockId;
use crate::mir::{
    CallArgument, CallContinuation, LocalId, Operand, Origin, Place, ScopeId, Statement, Terminator,
};
use crate::semantic::ExprId;

#[derive(Debug)]
struct PendingBlock {
    scope: ScopeId,
    statements: Vec<Statement>,
    terminator: Option<Terminator>,
}

/// Owns the transient distinction between an open block and completed MIR.
///
/// `BasicBlock` itself always has a terminator. Keeping the incomplete state
/// here prevents placeholder terminators from entering checked MIR merely to
/// make expression lowering able to split control flow.
pub(super) struct ControlFlowBuilder {
    blocks: Vec<PendingBlock>,
    current: Option<BasicBlockId>,
}

impl ControlFlowBuilder {
    pub(super) fn new(root_scope: ScopeId) -> Self {
        Self {
            blocks: vec![PendingBlock {
                scope: root_scope,
                statements: Vec::new(),
                terminator: None,
            }],
            current: Some(BasicBlockId::from_index(0)),
        }
    }

    pub(super) fn push_statement(&mut self, statement: Statement) -> Result<(), BuildError> {
        self.current_block_mut()?.statements.push(statement);
        Ok(())
    }

    pub(super) fn terminate(&mut self, terminator: Terminator) -> Result<(), BuildError> {
        let current = self.current.take().ok_or(BuildError::MissingOpenBlock)?;
        let block = self
            .blocks
            .get_mut(current.index())
            .ok_or(BuildError::MissingOpenBlock)?;
        if block.terminator.replace(terminator).is_some() {
            return Err(BuildError::BlockAlreadyTerminated);
        }
        Ok(())
    }

    pub(super) fn reserve_block(&mut self, scope: ScopeId) -> BasicBlockId {
        let block = BasicBlockId::from_index(self.blocks.len());
        self.blocks.push(PendingBlock {
            scope,
            statements: Vec::new(),
            terminator: None,
        });
        block
    }

    pub(super) fn select_block(&mut self, block: BasicBlockId) -> Result<(), BuildError> {
        if self.current.is_some() {
            return Err(BuildError::OpenBlockNotTerminated);
        }
        let pending = self
            .blocks
            .get(block.index())
            .ok_or(BuildError::MissingOpenBlock)?;
        if pending.terminator.is_some() {
            return Err(BuildError::BlockAlreadyTerminated);
        }
        self.current = Some(block);
        Ok(())
    }

    pub(super) fn set_switch_join(
        &mut self,
        block: BasicBlockId,
        join_target: Option<BasicBlockId>,
    ) -> Result<(), BuildError> {
        let pending = self
            .blocks
            .get_mut(block.index())
            .ok_or(BuildError::MissingOpenBlock)?;
        let Some(Terminator::Switch {
            join_target: current,
            ..
        }) = pending.terminator.as_mut()
        else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
        *current = join_target;
        Ok(())
    }

    pub(super) fn discard_last_reserved_block(
        &mut self,
        block: BasicBlockId,
    ) -> Result<(), BuildError> {
        if self.current.is_some()
            || block.index() + 1 != self.blocks.len()
            || self.blocks[block.index()].terminator.is_some()
            || !self.blocks[block.index()].statements.is_empty()
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        self.blocks.pop();
        Ok(())
    }

    pub(super) fn current_block(&self) -> Result<BasicBlockId, BuildError> {
        self.current.ok_or(BuildError::MissingOpenBlock)
    }

    fn current_scope(&self) -> Result<ScopeId, BuildError> {
        let current = self.current_block()?;
        self.blocks
            .get(current.index())
            .map(|block| block.scope)
            .ok_or(BuildError::MissingOpenBlock)
    }

    pub(super) fn emit_returning_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        destination: LocalId,
    ) -> Result<(), BuildError> {
        let target = self.reserve_block(self.current_scope()?);
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Return {
                destination: Place::local(destination),
                target,
            },
        })?;
        self.select_block(target)
    }

    pub(super) fn emit_effect_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
    ) -> Result<(), BuildError> {
        let target = self.reserve_block(self.current_scope()?);
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Continue { target },
        })?;
        self.select_block(target)
    }

    pub(super) fn emit_drop(
        &mut self,
        place: Place,
        plan: crate::mir::DropPlanId,
    ) -> Result<(), BuildError> {
        let target = self.reserve_block(self.current_scope()?);
        self.terminate(Terminator::Drop {
            place,
            plan,
            target,
        })?;
        self.select_block(target)
    }

    pub(super) fn emit_never_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
    ) -> Result<(), BuildError> {
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Never,
        })
    }

    pub(super) fn emit_trapping_outcome_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        destination: LocalId,
    ) -> Result<(), BuildError> {
        self.emit_outcome_call(source, callee, arguments, destination, Terminator::Trap)
    }

    pub(super) fn emit_propagating_outcome_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        destination: LocalId,
    ) -> Result<(), BuildError> {
        self.emit_outcome_call(
            source,
            callee,
            arguments,
            destination,
            Terminator::PropagateFailure,
        )
    }

    pub(super) fn emit_trapping_outcome_effect(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
    ) -> Result<(), BuildError> {
        self.emit_outcome_effect(source, callee, arguments, Terminator::Trap)
    }

    pub(super) fn emit_propagating_outcome_effect(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
    ) -> Result<(), BuildError> {
        self.emit_outcome_effect(source, callee, arguments, Terminator::PropagateFailure)
    }

    pub(super) fn begin_handled_outcome_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        destination: LocalId,
        failure_scope: ScopeId,
        failure_payload: Option<LocalId>,
    ) -> Result<BasicBlockId, BuildError> {
        let success = self.reserve_block(self.current_scope()?);
        let failure = self.reserve_block(failure_scope);
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Outcome {
                destination: Place::local(destination),
                success,
                failure,
                failure_payload,
            },
        })?;
        self.select_block(failure)?;
        Ok(success)
    }

    pub(super) fn begin_stored_outcome_inspection(
        &mut self,
        origin: Origin,
        source: Operand,
        layer: crate::outcomes::OutcomeLayer,
        destination: LocalId,
        failure_scope: ScopeId,
        failure_payload: Option<LocalId>,
    ) -> Result<BasicBlockId, BuildError> {
        let success = self.reserve_block(self.current_scope()?);
        let failure = self.reserve_block(failure_scope);
        self.terminate(Terminator::InspectOutcome {
            origin,
            source,
            layer,
            destination: Place::local(destination),
            success,
            failure,
            failure_payload,
        })?;
        self.select_block(failure)?;
        Ok(success)
    }

    pub(super) fn emit_stored_outcome_inspection(
        &mut self,
        origin: Origin,
        source: Operand,
        layer: crate::outcomes::OutcomeLayer,
        destination: LocalId,
        failure_terminator: Terminator,
    ) -> Result<(), BuildError> {
        debug_assert!(matches!(
            failure_terminator,
            Terminator::Trap | Terminator::PropagateFailure
        ));
        let scope = self.current_scope()?;
        let success = self.reserve_block(scope);
        let failure = self.reserve_block(scope);
        self.terminate(Terminator::InspectOutcome {
            origin,
            source,
            layer,
            destination: Place::local(destination),
            success,
            failure,
            failure_payload: None,
        })?;
        self.select_block(failure)?;
        self.terminate(failure_terminator)?;
        self.select_block(success)
    }

    fn emit_outcome_call(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        destination: LocalId,
        failure_terminator: Terminator,
    ) -> Result<(), BuildError> {
        debug_assert!(matches!(
            failure_terminator,
            Terminator::Trap | Terminator::PropagateFailure
        ));
        let scope = self.current_scope()?;
        let success = self.reserve_block(scope);
        let failure = self.reserve_block(scope);
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Outcome {
                destination: Place::local(destination),
                success,
                failure,
                failure_payload: None,
            },
        })?;
        self.select_block(failure)?;
        self.terminate(failure_terminator)?;
        self.select_block(success)
    }

    fn emit_outcome_effect(
        &mut self,
        source: ExprId,
        callee: crate::mir::CallInstance,
        arguments: Vec<CallArgument>,
        failure_terminator: Terminator,
    ) -> Result<(), BuildError> {
        debug_assert!(matches!(
            failure_terminator,
            Terminator::Trap | Terminator::PropagateFailure
        ));
        let scope = self.current_scope()?;
        let success = self.reserve_block(scope);
        let failure = self.reserve_block(scope);
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::OutcomeEffect {
                success,
                failure,
                failure_payload: None,
            },
        })?;
        self.select_block(failure)?;
        self.terminate(failure_terminator)?;
        self.select_block(success)
    }

    pub(super) fn finish(self) -> Result<Vec<crate::mir::model::BasicBlock>, BuildError> {
        if self.current.is_some() {
            return Err(BuildError::OpenBlockNotTerminated);
        }
        self.blocks
            .into_iter()
            .map(|block| {
                Ok(crate::mir::model::BasicBlock {
                    scope: block.scope,
                    statements: block.statements,
                    terminator: block
                        .terminator
                        .ok_or(BuildError::UnterminatedReservedBlock)?,
                })
            })
            .collect()
    }

    fn current_block_mut(&mut self) -> Result<&mut PendingBlock, BuildError> {
        let current = self.current.ok_or(BuildError::MissingOpenBlock)?;
        self.blocks
            .get_mut(current.index())
            .ok_or(BuildError::MissingOpenBlock)
    }
}
