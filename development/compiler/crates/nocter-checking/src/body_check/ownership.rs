use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{ArenaBuilder, BodyNodeId, BodyScopeId, CallableCapability, LoopId, TypeStore};
use nocter_source_index::SourceOrigin;

mod cleanup;

use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::{Copyability, CopyabilityTable};
use crate::ownership::{MovePath, OwnershipState, OwnershipStateError, initialized_body_roots};
use crate::{
    AggregateConstruction, BodySource, CallTarget, CheckedBody, CheckedCall, CheckedControl,
    CheckedOperation, CheckedOutcome, CleanupAction, CleanupSchedule, CleanupTable, CleanupTiming,
    DropTable, LoopKind, PlaceAccess, PrimitiveOperation,
};
use cleanup::CleanupPlanner;

/// Validates flow-dependent ownership after typed HIR construction.
pub(super) fn analyze_body_ownership(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    copyabilities: &mut CopyabilityTable,
    drops: &DropTable,
    source: BodySource<'_>,
    body: &CheckedBody,
    origins: &HashMap<BodyNodeId, SourceOrigin>,
) -> Result<CleanupTable, BodyCheckError> {
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
        drops,
        source,
        body,
        origins,
        loops: Vec::new(),
        scopes: Vec::new(),
        cleanup_schedules: HashMap::new(),
    };
    analyzer.visit(body.root(), &mut state)?;
    analyzer.validate_all_copies()?;
    analyzer.finish_cleanups().map_err(Into::into)
}

struct OwnershipAnalyzer<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut TypeStore,
    copyabilities: &'program mut CopyabilityTable,
    drops: &'program DropTable,
    source: BodySource<'program>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    loops: Vec<LoopFlow>,
    scopes: Vec<BodyScopeId>,
    cleanup_schedules: HashMap<BodyNodeId, CleanupSchedule>,
}

struct LoopFlow {
    id: LoopId,
    body_scope: BodyScopeId,
    breaks: Vec<OwnershipState>,
    continues: Vec<OwnershipState>,
}

impl OwnershipAnalyzer<'_> {
    fn finish_cleanups(mut self) -> Result<CleanupTable, BodyCheckInternalError> {
        if !self.scopes.is_empty() || !self.loops.is_empty() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let mut schedules = ArenaBuilder::new();
        for (node, _) in self.body.nodes().iter() {
            let actual = schedules.insert(self.cleanup_schedules.remove(&node));
            if actual != node {
                return Err(BodyCheckInternalError::CleanupPlanning);
            }
        }
        if !self.cleanup_schedules.is_empty() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        Ok(CleanupTable::new(schedules.finish()))
    }

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
                self.visit_place_use(node, *place, state)
            }
            CheckedOperation::Copy(place) => {
                if !self.visit_place_use(node, *place, state)? {
                    return Ok(false);
                }
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
            CheckedOperation::Comparison(comparison) => {
                if !self.visit(comparison.left().value(), state)? {
                    return Ok(false);
                }
                self.visit(comparison.right().value(), state)
            }
            CheckedOperation::Control(control) => self.visit_control(node, control, state),
            CheckedOperation::Call(call) => self.visit_call(call, state),
            CheckedOperation::BorrowConversion(conversion) => self.visit(conversion.value(), state),
            CheckedOperation::Aggregate(aggregate) => self.visit_aggregate(aggregate, state),
            CheckedOperation::Outcome(
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

    fn visit_aggregate(
        &mut self,
        aggregate: &AggregateConstruction,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        match aggregate {
            AggregateConstruction::Struct { fields, .. } => {
                for (_, value) in fields {
                    if !self.visit(*value, state)? {
                        return Ok(false);
                    }
                }
            }
            AggregateConstruction::Enum { payload, .. }
            | AggregateConstruction::FixedArray(payload) => {
                for value in payload {
                    if !self.visit(*value, state)? {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    fn visit_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        match control {
            CheckedControl::Block {
                scope,
                statements,
                result,
            } => self.visit_block(node, *scope, statements, *result, state),
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
            CheckedControl::Discard(value) => self.visit_discard(node, *value, state),
            CheckedControl::Return(value) => {
                if let Some(value) = value
                    && !self.visit(*value, state)?
                {
                    return Ok(false);
                }
                let actions = self.all_scope_cleanup(state)?;
                self.record_cleanup(node, CleanupTiming::BeforeTransfer, actions);
                Ok(false)
            }
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => self.visit_if(*condition, *then_branch, *else_branch, state),
            CheckedControl::Logical { left, right, .. } => self.visit_logical(*left, *right, state),
            CheckedControl::Unreachable(_) => Ok(false),
            CheckedControl::Break(loop_) => self.visit_loop_control(node, *loop_, true, state),
            CheckedControl::Continue(loop_) => self.visit_loop_control(node, *loop_, false, state),
            CheckedControl::Drop(place) => self.visit_drop(node, *place, state),
            CheckedControl::Assign { target, value } => {
                self.visit_assignment(node, *target, *value, state)
            }
            CheckedControl::CompoundAssign { target, value, .. } => {
                self.visit_compound_assignment(node, *target, *value, state)
            }
            CheckedControl::Loop(loop_) => self.visit_loop(*loop_, state),
            CheckedControl::Match { .. } | CheckedControl::Region { .. } => {
                Err(BodyCheckInternalError::UnsupportedOwnershipOperation(node).into())
            }
        }
    }

    fn visit_call(
        &mut self,
        call: &CheckedCall,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if let CallTarget::CallableValue {
            value, capability, ..
        } = call.target()
        {
            if !self.visit(*value, state)? {
                return Ok(false);
            }
            if *capability == CallableCapability::Owned {
                let place = self
                    .body
                    .nodes()
                    .get(*value)
                    .and_then(|node| match node.operation() {
                        CheckedOperation::Place(place) => Some(*place),
                        _ => None,
                    })
                    .ok_or(BodyCheckInternalError::UnsupportedOwnershipOperation(
                        *value,
                    ))?;
                let path = self.move_path(place)?;
                state
                    .move_out(&path)
                    .map_err(|_| BodyCheckInternalError::OwnershipState)?;
            }
        }
        if let Some(receiver) = call.receiver()
            && !self.visit(receiver.value(), state)?
        {
            return Ok(false);
        }
        for argument in call.arguments() {
            if !self.visit(*argument, state)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn visit_block(
        &mut self,
        node: BodyNodeId,
        scope: BodyScopeId,
        statements: &[BodyNodeId],
        result: Option<BodyNodeId>,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        self.enter_scope(scope)?;
        let mut reaches = true;
        for statement in statements {
            if !self.visit(*statement, state)? {
                reaches = false;
                break;
            }
        }
        if reaches && let Some(result) = result {
            reaches = self.visit(result, state)?;
        }
        if reaches {
            let mut actions = self.scope_cleanup(scope, state)?;
            if self.scopes.len() == 1 {
                actions.extend(self.parameter_cleanup(state)?);
            }
            self.record_cleanup(node, CleanupTiming::BeforeTransfer, actions);
        }
        self.leave_scope(scope)?;
        Ok(reaches)
    }

    fn visit_discard(
        &mut self,
        node: BodyNodeId,
        value: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(value, state)? {
            return Ok(false);
        }
        let ty = self
            .body
            .nodes()
            .get(value)
            .map(crate::CheckedNode::ty)
            .ok_or(BodyCheckInternalError::MissingNode(value))?;
        if let Some(action) = self.value_cleanup(value, ty)? {
            self.record_cleanup(node, CleanupTiming::BeforeTransfer, vec![action]);
        }
        Ok(true)
    }

    fn visit_if(
        &mut self,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(condition, state)? {
            return Ok(false);
        }
        let entry = state.clone();
        let mut incoming = Vec::new();
        let mut then_state = entry.clone();
        if self.visit(then_branch, &mut then_state)? {
            incoming.push(then_state);
        }
        let mut else_state = entry.clone();
        if let Some(else_branch) = else_branch {
            if self.visit(else_branch, &mut else_state)? {
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

    fn visit_logical(
        &mut self,
        left: BodyNodeId,
        right: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(left, state)? {
            return Ok(false);
        }
        let entry = state.clone();
        let mut incoming = vec![entry.clone()];
        let mut right_state = entry.clone();
        if self.visit(right, &mut right_state)? {
            incoming.push(right_state);
        }
        *state = entry
            .join_reachable(&incoming)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        Ok(true)
    }

    fn visit_loop_control(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        is_break: bool,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let Some(frame) = self.loops.last().filter(|frame| frame.id == loop_) else {
            return Err(BodyCheckInternalError::LoopStack.into());
        };
        let actions = self.loop_scope_cleanup(frame.body_scope, state)?;
        self.record_cleanup(node, CleanupTiming::BeforeTransfer, actions);
        let frame = self
            .loops
            .last_mut()
            .ok_or(BodyCheckInternalError::LoopStack)?;
        if is_break {
            frame.breaks.push(state.clone());
        } else {
            frame.continues.push(state.clone());
        }
        Ok(false)
    }

    fn visit_drop(
        &mut self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let path = self.move_path(place)?;
        self.require_path_initialized(node, &path, state)?;
        let ty = self
            .body
            .places()
            .get(place)
            .map(crate::CheckedPlace::ty)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let action = self.explicit_path_cleanup(&path, ty)?;
        state
            .move_out(&path)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        self.record_cleanup(node, CleanupTiming::BeforeTransfer, vec![action]);
        Ok(true)
    }

    fn visit_assignment(
        &mut self,
        node: BodyNodeId,
        target: nocter_model::PlaceId,
        value: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(value, state)? {
            return Ok(false);
        }
        let place = self
            .body
            .places()
            .get(target)
            .cloned()
            .ok_or(BodyCheckInternalError::InvalidMovePlace(target))?;
        if !self.visit_place_evaluation(&place, state)? {
            return Ok(false);
        }
        if place.has_dynamic_evaluation() || place.access() != PlaceAccess::Owned {
            self.require_path_initialized(node, &MovePath::initialized_base(&place), state)?;
        }
        let actions = match place.access() {
            PlaceAccess::Owned => {
                if let Some(path) = MovePath::from_place(&place) {
                    let actions = self.replacement_path_cleanup(&path, place.ty(), state)?;
                    if let Err(error) = state.assign(&path) {
                        return match error {
                            OwnershipStateError::UnavailableAssignmentParent { .. } => {
                                Err(self.rule(BodyRule::InvalidReinitialization, node)?)
                            }
                            OwnershipStateError::DuplicatePath(_)
                            | OwnershipStateError::UnknownPath(_)
                            | OwnershipStateError::NotInitialized { .. } => {
                                Err(BodyCheckInternalError::OwnershipState.into())
                            }
                        };
                    }
                    actions
                } else {
                    self.replacement_place_cleanup(target, place.ty())?
                        .into_iter()
                        .collect()
                }
            }
            PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite) => self
                .replacement_place_cleanup(target, place.ty())?
                .into_iter()
                .collect(),
            PlaceAccess::Borrowed(nocter_model::BorrowCapability::Readonly) => {
                return Err(BodyCheckInternalError::OwnershipState.into());
            }
        };
        self.record_cleanup(node, CleanupTiming::BeforeStore, actions);
        Ok(true)
    }

    fn visit_compound_assignment(
        &mut self,
        node: BodyNodeId,
        target: nocter_model::PlaceId,
        value: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(value, state)? {
            return Ok(false);
        }
        let place = self
            .body
            .places()
            .get(target)
            .cloned()
            .ok_or(BodyCheckInternalError::InvalidMovePlace(target))?;
        if !self.visit_place_evaluation(&place, state)? {
            return Ok(false);
        }
        let required =
            MovePath::from_place(&place).unwrap_or_else(|| MovePath::initialized_base(&place));
        self.require_path_initialized(node, &required, state)?;
        Ok(true)
    }

    fn visit_place_evaluation(
        &mut self,
        place: &crate::CheckedPlace,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let nodes = place.evaluation_nodes().collect::<Vec<_>>();
        for node in nodes {
            if !self.visit(node, state)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn visit_place_use(
        &mut self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let place = self
            .body
            .places()
            .get(place)
            .cloned()
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        if !self.visit_place_evaluation(&place, state)? {
            return Ok(false);
        }
        let required =
            MovePath::from_place(&place).unwrap_or_else(|| MovePath::initialized_base(&place));
        self.require_path_initialized(node, &required, state)?;
        Ok(true)
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
        let body_scope = self.block_scope(definition.body())?;
        let preheader = state.clone();
        let mut header = preheader.clone();
        loop {
            self.loops.push(LoopFlow {
                id: loop_,
                body_scope,
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

    fn enter_scope(&mut self, scope: BodyScopeId) -> Result<(), BodyCheckInternalError> {
        let expected_parent = self.scopes.last().copied();
        let actual_parent = self
            .body
            .scopes()
            .get(scope)
            .copied()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?
            .parent();
        if actual_parent != expected_parent {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        self.scopes.push(scope);
        Ok(())
    }

    fn leave_scope(&mut self, scope: BodyScopeId) -> Result<(), BodyCheckInternalError> {
        if self.scopes.pop() != Some(scope) {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        Ok(())
    }

    fn block_scope(&self, node: BodyNodeId) -> Result<BodyScopeId, BodyCheckInternalError> {
        match self
            .body
            .nodes()
            .get(node)
            .map(crate::CheckedNode::operation)
        {
            Some(CheckedOperation::Control(CheckedControl::Block { scope, .. })) => Ok(*scope),
            Some(_) | None => Err(BodyCheckInternalError::CleanupPlanning),
        }
    }

    fn scope_cleanup(
        &mut self,
        scope: BodyScopeId,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .scope_actions(scope, state)
    }

    fn parameter_cleanup(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .parameter_actions(state)
    }

    fn value_cleanup(
        &mut self,
        node: BodyNodeId,
        ty: nocter_model::TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .value_action(node, ty)
    }

    fn explicit_path_cleanup(
        &mut self,
        path: &MovePath,
        ty: nocter_model::TypeId,
    ) -> Result<CleanupAction, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .explicit_path_action(path, ty)
    }

    fn replacement_path_cleanup(
        &mut self,
        path: &MovePath,
        ty: nocter_model::TypeId,
        state: &OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .replacement_path_actions(path, ty, state)
    }

    fn replacement_place_cleanup(
        &mut self,
        place: nocter_model::PlaceId,
        ty: nocter_model::TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        )
        .replacement_place_action(place, ty)
    }

    fn all_scope_cleanup(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let scopes = self.scopes.clone();
        let mut actions = Vec::new();
        for scope in scopes.into_iter().rev() {
            actions.extend(self.scope_cleanup(scope, state)?);
        }
        actions.extend(self.parameter_cleanup(state)?);
        Ok(actions)
    }

    fn loop_scope_cleanup(
        &mut self,
        body_scope: BodyScopeId,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let Some(target) = self.scopes.iter().rposition(|scope| *scope == body_scope) else {
            return Err(BodyCheckInternalError::LoopStack);
        };
        let scopes = self.scopes[target..].to_vec();
        let mut actions = Vec::new();
        for scope in scopes.into_iter().rev() {
            actions.extend(self.scope_cleanup(scope, state)?);
        }
        Ok(actions)
    }

    fn record_cleanup(
        &mut self,
        node: BodyNodeId,
        timing: CleanupTiming,
        actions: Vec<CleanupAction>,
    ) {
        if actions.is_empty() {
            self.cleanup_schedules.remove(&node);
        } else {
            self.cleanup_schedules
                .insert(node, CleanupSchedule::new(timing, actions));
        }
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
            Copyability::MoveOnly => Err(self.rule(BodyRule::ImplicitMove, node)?),
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
                Err(self.rule(BodyRule::UninitializedPlace, node)?)
            }
            Err(
                OwnershipStateError::DuplicatePath(_)
                | OwnershipStateError::UnknownPath(_)
                | OwnershipStateError::UnavailableAssignmentParent { .. },
            ) => Err(BodyCheckInternalError::OwnershipState.into()),
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
    ) -> Result<BodyCheckError, BodyCheckInternalError> {
        let origin = self
            .origins
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))?;
        Ok(BodyCheckError::from_rule(rule, rule.diagnostic(origin)))
    }
}
