//! Construction-only state for splitting scalar expressions into MIR blocks.

use super::BuildError;
use crate::mir::ids::BasicBlockId;
use crate::mir::{CallArgument, CallContinuation, LocalId, Origin, Place, Statement, Terminator};
use crate::semantic::{DefId, ExprId};

#[derive(Debug, Default)]
struct PendingBlock {
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
    pub(super) fn new() -> Self {
        Self {
            blocks: vec![PendingBlock::default()],
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

    pub(super) fn reserve_block(&mut self) -> BasicBlockId {
        let block = BasicBlockId::from_index(self.blocks.len());
        self.blocks.push(PendingBlock::default());
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

    pub(super) fn current_block(&self) -> Result<BasicBlockId, BuildError> {
        self.current.ok_or(BuildError::MissingOpenBlock)
    }

    pub(super) fn emit_returning_call(
        &mut self,
        source: ExprId,
        callee: DefId,
        arguments: Vec<CallArgument>,
        destination: LocalId,
    ) -> Result<(), BuildError> {
        let target = self.reserve_block();
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Return {
                destination: Place { local: destination },
                target,
            },
        })?;
        self.select_block(target)
    }

    pub(super) fn emit_never_call(
        &mut self,
        source: ExprId,
        callee: DefId,
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
        callee: DefId,
        arguments: Vec<CallArgument>,
        destination: LocalId,
    ) -> Result<(), BuildError> {
        self.emit_outcome_call(source, callee, arguments, destination, Terminator::Trap)
    }

    pub(super) fn emit_propagating_outcome_call(
        &mut self,
        source: ExprId,
        callee: DefId,
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

    fn emit_outcome_call(
        &mut self,
        source: ExprId,
        callee: DefId,
        arguments: Vec<CallArgument>,
        destination: LocalId,
        failure_terminator: Terminator,
    ) -> Result<(), BuildError> {
        debug_assert!(matches!(
            failure_terminator,
            Terminator::Trap | Terminator::PropagateFailure
        ));
        let success = self.reserve_block();
        let failure = self.reserve_block();
        self.terminate(Terminator::Call {
            origin: Origin::Expression(source),
            callee,
            arguments,
            continuation: CallContinuation::Outcome {
                destination: Place { local: destination },
                success,
                failure,
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
