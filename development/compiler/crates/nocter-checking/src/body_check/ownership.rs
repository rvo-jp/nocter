use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{BodyNodeId, LoopId, TypeStore};
use nocter_source_index::SourceOrigin;

use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::{Copyability, CopyabilityTable};
use crate::ownership::{MovePath, OwnershipState, OwnershipStateError, initialized_body_roots};
use crate::{
    BodySource, CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome, LoopKind,
    PrimitiveOperation,
};

/// Validates flow-dependent ownership after typed HIR construction.
pub(super) fn analyze_body_ownership(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    copyabilities: &mut CopyabilityTable,
    source: BodySource<'_>,
    body: &CheckedBody,
    origins: &HashMap<BodyNodeId, SourceOrigin>,
) -> Result<(), BodyCheckError> {
    let mut state = OwnershipState::default();
    for root in initialized_body_roots(graph, source)
        .ok_or(BodyCheckInternalError::BodyIdentityMismatch(source.body()))?
    {
        state
            .declare_initialized(MovePath::root(root))
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
    }
    let mut analyzer = OwnershipAnalyzer {
        graph,
        types,
        copyabilities,
        body,
        origins,
        loops: Vec::new(),
    };
    analyzer.visit(body.root(), &mut state)?;
    analyzer.validate_all_copies()?;
    Ok(())
}

struct OwnershipAnalyzer<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut TypeStore,
    copyabilities: &'program mut CopyabilityTable,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    loops: Vec<LoopFlow>,
}

struct LoopFlow {
    id: LoopId,
    breaks: Vec<OwnershipState>,
    continues: Vec<OwnershipState>,
}

impl OwnershipAnalyzer<'_> {
    fn validate_all_copies(&mut self) -> Result<(), BodyCheckError> {
        for (node, checked) in self.body.nodes().iter() {
            if matches!(checked.operation(), CheckedOperation::Copy(_)) {
                self.validate_copy(node, checked.ty())?;
            }
        }
        Ok(())
    }

    fn visit(
        &mut self,
        node: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        match checked.operation() {
            CheckedOperation::Complete
            | CheckedOperation::Constant(_)
            | CheckedOperation::Outcome(CheckedOutcome::Absent) => Ok(true),
            CheckedOperation::Place(place) | CheckedOperation::Borrow { place, .. } => {
                self.require_initialized(node, *place, state)?;
                Ok(true)
            }
            CheckedOperation::Copy(place) => {
                self.require_initialized(node, *place, state)?;
                self.validate_copy(node, checked.ty())?;
                Ok(true)
            }
            CheckedOperation::Move(place) => {
                let path = self.move_path(*place)?;
                self.require_path_initialized(node, &path, state)?;
                state
                    .move_out(&path)
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
                Ok(true)
            }
            CheckedOperation::Outcome(
                CheckedOutcome::Inject { payload, .. } | CheckedOutcome::Failure(payload),
            ) => self.visit(*payload, state),
            CheckedOperation::Primitive(
                PrimitiveOperation::Unary { operand, .. }
                | PrimitiveOperation::IntegerConversion { operand, .. },
            ) => self.visit(*operand, state),
            CheckedOperation::Primitive(PrimitiveOperation::Binary { left, right, .. }) => {
                if !self.visit(*left, state)? {
                    return Ok(false);
                }
                self.visit(*right, state)
            }
            CheckedOperation::Control(control) => self.visit_control(node, control, state),
            CheckedOperation::Call(_)
            | CheckedOperation::Coerce { .. }
            | CheckedOperation::Aggregate(_)
            | CheckedOperation::Outcome(
                CheckedOutcome::Propagate { .. } | CheckedOutcome::Recover { .. },
            )
            | CheckedOperation::Closure(_)
            | CheckedOperation::Sequence(_)
            | CheckedOperation::StringLiteral { .. }
            | CheckedOperation::Interpolation(_) => {
                Err(BodyCheckInternalError::UnsupportedOwnershipOperation(node).into())
            }
        }
    }

    fn visit_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        match control {
            CheckedControl::Block { statements, result } => {
                for statement in statements {
                    if !self.visit(*statement, state)? {
                        return Ok(false);
                    }
                }
                result.map_or(Ok(true), |result| self.visit(result, state))
            }
            CheckedControl::Bind {
                binding,
                initializer,
            } => {
                if !self.visit(*initializer, state)? {
                    return Ok(false);
                }
                state
                    .declare_initialized(MovePath::root(crate::PlaceRoot::Local(*binding)))
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
                Ok(true)
            }
            CheckedControl::Discard(value) => self.visit(*value, state),
            CheckedControl::Return(value) => {
                if let Some(value) = value {
                    self.visit(*value, state)?;
                }
                Ok(false)
            }
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if !self.visit(*condition, state)? {
                    return Ok(false);
                }
                let entry = state.clone();
                let mut incoming = Vec::new();
                let mut then_state = entry.clone();
                if self.visit(*then_branch, &mut then_state)? {
                    incoming.push(then_state);
                }
                let mut else_state = entry.clone();
                if let Some(else_branch) = else_branch {
                    if self.visit(*else_branch, &mut else_state)? {
                        incoming.push(else_state);
                    }
                } else {
                    incoming.push(else_state);
                }
                *state = entry
                    .join_reachable(&incoming)
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
                Ok(!incoming.is_empty())
            }
            CheckedControl::Unreachable(_) => Ok(false),
            CheckedControl::Break(loop_) => {
                let Some(frame) = self.loops.last_mut().filter(|frame| frame.id == *loop_) else {
                    return Err(BodyCheckInternalError::LoopStack.into());
                };
                frame.breaks.push(state.clone());
                Ok(false)
            }
            CheckedControl::Continue(loop_) => {
                let Some(frame) = self.loops.last_mut().filter(|frame| frame.id == *loop_) else {
                    return Err(BodyCheckInternalError::LoopStack.into());
                };
                frame.continues.push(state.clone());
                Ok(false)
            }
            CheckedControl::Loop(loop_) => self.visit_loop(*loop_, state),
            CheckedControl::Assign { .. }
            | CheckedControl::CompoundAssign { .. }
            | CheckedControl::Drop(_)
            | CheckedControl::Match { .. }
            | CheckedControl::Region { .. } => {
                Err(BodyCheckInternalError::UnsupportedOwnershipOperation(node).into())
            }
        }
    }

    fn visit_loop(
        &mut self,
        loop_: LoopId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let definition = self
            .body
            .loops()
            .get(loop_)
            .cloned()
            .ok_or(BodyCheckInternalError::UnknownLoop(loop_))?;
        if let LoopKind::Range { start, end, .. } = definition.kind()
            && (!self.visit(*start, state)? || !self.visit(*end, state)?)
        {
            return Ok(false);
        }
        let preheader = state.clone();
        let mut header = preheader.clone();
        loop {
            self.loops.push(LoopFlow {
                id: loop_,
                breaks: Vec::new(),
                continues: Vec::new(),
            });
            let mut iteration = header.clone();
            let condition_reaches = match definition.kind() {
                LoopKind::While { condition } => self.visit(*condition, &mut iteration)?,
                LoopKind::Infinite | LoopKind::Range { .. } => true,
                LoopKind::For { .. } => {
                    return Err(BodyCheckInternalError::UnsupportedLoop(loop_).into());
                }
            };
            let condition_exit = (condition_reaches
                && matches!(
                    definition.kind(),
                    LoopKind::While { .. } | LoopKind::Range { .. }
                ))
            .then(|| iteration.clone());
            if condition_reaches && let LoopKind::Range { binding, .. } = definition.kind() {
                iteration
                    .declare_initialized(MovePath::root(crate::PlaceRoot::Local(*binding)))
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
            }
            let body_reaches =
                condition_reaches && self.visit(definition.body(), &mut iteration)?;
            let mut frame = self.loops.pop().ok_or(BodyCheckInternalError::LoopStack)?;
            if frame.id != loop_ {
                return Err(BodyCheckInternalError::LoopStack.into());
            }
            if body_reaches {
                frame.continues.push(iteration);
            }
            let mut header_incoming = Vec::with_capacity(frame.continues.len() + 1);
            header_incoming.push(preheader.clone());
            header_incoming.extend(frame.continues);
            let next_header = preheader
                .join_reachable(&header_incoming)
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
            if next_header != header {
                header = next_header;
                continue;
            }

            let mut exits = frame.breaks;
            if let Some(condition_exit) = condition_exit {
                exits.push(condition_exit);
            }
            *state = preheader
                .join_reachable(&exits)
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
            return Ok(!exits.is_empty());
        }
    }

    fn require_initialized(
        &self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        state: &OwnershipState,
    ) -> Result<(), BodyCheckError> {
        let path = self.move_path(place)?;
        self.require_path_initialized(node, &path, state)
    }

    fn validate_copy(
        &mut self,
        node: BodyNodeId,
        ty: nocter_model::TypeId,
    ) -> Result<(), BodyCheckError> {
        match self
            .copyabilities
            .classify(self.graph, self.types, ty)
            .map_err(BodyCheckInternalError::Copyability)?
        {
            Copyability::Copy => Ok(()),
            Copyability::MoveOnly => Err(self.rule(BodyRule::ImplicitMove, node)?.into()),
        }
    }

    fn require_path_initialized(
        &self,
        node: BodyNodeId,
        path: &MovePath,
        state: &OwnershipState,
    ) -> Result<(), BodyCheckError> {
        match state.require_initialized(path) {
            Ok(()) => Ok(()),
            Err(OwnershipStateError::NotInitialized { .. }) => {
                Err(self.rule(BodyRule::UninitializedPlace, node)?.into())
            }
            Err(OwnershipStateError::DuplicatePath(_) | OwnershipStateError::UnknownPath(_)) => {
                Err(BodyCheckInternalError::OwnershipState.into())
            }
        }
    }

    fn move_path(&self, place: nocter_model::PlaceId) -> Result<MovePath, BodyCheckInternalError> {
        self.body
            .places()
            .get(place)
            .and_then(MovePath::from_place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))
    }

    fn rule(
        &self,
        rule: BodyRule,
        node: BodyNodeId,
    ) -> Result<nocter_diagnostics::SourceDiagnostic, BodyCheckInternalError> {
        let origin = self
            .origins
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))?;
        Ok(rule.diagnostic(origin))
    }
}
