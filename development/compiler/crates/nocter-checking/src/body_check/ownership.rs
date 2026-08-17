use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{
    ArenaBuilder, BodyNodeId, BodyScopeId, BuiltinType, CallableCapability, LocalBindingId, LoopId,
    TypeStore,
};
use nocter_source_index::SourceOrigin;

mod cleanup;
mod interpolation;
mod outcomes;
mod patterns;
mod sequences;
mod temporaries;

use super::diagnostic::BodyRule;
use super::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::{Copyability, CopyabilityTable};
use crate::ownership::{
    MovePath, OwnershipState, OwnershipStateError, TemporaryIdentity, initialized_body_roots,
};
use crate::{
    AggregateConstruction, BodySource, CheckedBody, CheckedControl, CheckedOperation,
    CheckedOutcome, CleanupAction, CleanupCondition, CleanupSchedule, CleanupTable, CleanupTarget,
    CleanupTiming, ClosureDefinition, ClosureTable, DropTable, LoopKind, PlaceAccess,
    PrimitiveOperation, ReadonlyOperandPreparation,
};
use cleanup::CleanupPlanner;
use temporaries::TemporaryPlanner;

/// Validates flow-dependent ownership after typed HIR construction.
pub(super) fn analyze_body_ownership(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    copyabilities: &mut CopyabilityTable,
    drops: &DropTable,
    closures: &ClosureTable,
    input: OwnershipBodyInput<'_>,
) -> Result<CleanupTable, BodyCheckError> {
    let OwnershipBodyInput {
        source,
        body,
        origins,
    } = input;
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
        regions: Vec::new(),
        temporaries: TemporaryPlanner::default(),
        cleanup_schedules: HashMap::new(),
        closure: None,
    };
    analyzer.visit(body.root(), &mut state)?;
    for (_, definition) in closures
        .definitions()
        .iter()
        .filter(|(_, definition)| definition.owner() == source.body())
    {
        if !analyzer.scopes.is_empty() || !analyzer.loops.is_empty() || !analyzer.regions.is_empty()
        {
            return Err(BodyCheckInternalError::CleanupPlanning.into());
        }
        let mut state = OwnershipState::default();
        for parameter in definition.parameters() {
            state
                .declare_initialized(MovePath::root(crate::PlaceRoot::Local(*parameter)))
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        for capture in definition.environment().iter().copied() {
            state
                .declare_initialized(MovePath::root(crate::PlaceRoot::Capture(capture.binding())))
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        analyzer.closure = Some(definition);
        analyzer.visit(definition.body(), &mut state)?;
        analyzer.closure = None;
    }
    analyzer.validate_all_copies()?;
    analyzer.finish_cleanups().map_err(Into::into)
}

#[derive(Clone, Copy)]
pub(super) struct OwnershipBodyInput<'program> {
    source: BodySource<'program>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program> OwnershipBodyInput<'program> {
    pub(super) fn new(
        source: BodySource<'program>,
        body: &'program CheckedBody,
        origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    ) -> Self {
        Self {
            source,
            body,
            origins,
        }
    }
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
    regions: Vec<RegionFlow>,
    temporaries: TemporaryPlanner,
    cleanup_schedules: HashMap<BodyNodeId, Vec<CleanupSchedule>>,
    closure: Option<&'program ClosureDefinition>,
}

struct LoopFlow {
    id: LoopId,
    body_scope: BodyScopeId,
    iterator: Option<TemporaryIdentity>,
    retained_temporaries: Vec<TemporaryIdentity>,
    breaks: Vec<OwnershipState>,
    continues: Vec<OwnershipState>,
}

#[derive(Clone, Copy)]
struct RegionFlow {
    binding: LocalBindingId,
    body_scope: BodyScopeId,
    parent: BodyNodeId,
}

impl OwnershipAnalyzer<'_> {
    fn finish_cleanups(mut self) -> Result<CleanupTable, BodyCheckInternalError> {
        if !self.scopes.is_empty() || !self.loops.is_empty() || !self.regions.is_empty() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let mut schedules = ArenaBuilder::new();
        for (node, _) in self.body.nodes().iter() {
            let actual = schedules.insert(
                self.cleanup_schedules
                    .remove(&node)
                    .unwrap_or_default()
                    .into_boxed_slice(),
            );
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
            | CheckedOperation::LiteralPackLength(_)
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
            CheckedOperation::Outcome(CheckedOutcome::Propagate { operand, .. }) => {
                self.visit_propagate(node, *operand, state)
            }
            CheckedOperation::Outcome(CheckedOutcome::Force { operand, .. }) => {
                self.visit(*operand, state)
            }
            CheckedOperation::Outcome(CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            }) => self.visit_recover(*operand, *binding, *fallback, state),
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
                if comparison.left().preparation() == ReadonlyOperandPreparation::BorrowTemporary {
                    self.activate_owned_temporary(comparison.left().value(), state)?;
                }
                if !self.visit(comparison.right().value(), state)? {
                    return Ok(false);
                }
                if comparison.right().preparation() == ReadonlyOperandPreparation::BorrowTemporary {
                    self.activate_owned_temporary(comparison.right().value(), state)?;
                }
                Ok(true)
            }
            CheckedOperation::Control(control) => self.visit_control(node, control, state),
            CheckedOperation::Call(call) => {
                let reaches = self.visit_call(call, state)?;
                Ok(reaches && checked.ty() != self.types.builtin(BuiltinType::Never))
            }
            CheckedOperation::BorrowConversion(conversion) => self.visit(conversion.value(), state),
            CheckedOperation::OpaqueWitness(witness) => self.visit(witness.value(), state),
            CheckedOperation::Aggregate(aggregate) => self.visit_aggregate(aggregate, state),
            CheckedOperation::Closure(closure) => self.visit_value_sequence(
                closure
                    .captures()
                    .iter()
                    .map(|capture| capture.initializer()),
                state,
            ),
            CheckedOperation::IteratorAcquisition(acquisition) => {
                self.visit_iterator_acquisition(acquisition, state)
            }
            CheckedOperation::Sequence(sequence) => self.visit_sequence(sequence, state),
            CheckedOperation::StringLiteral { allocation, .. } => {
                self.visit_allocation(*allocation, state)
            }
            CheckedOperation::Interpolation(interpolation) => {
                self.visit_interpolation(node, interpolation, state)
            }
        }
    }

    fn visit_allocation(
        &mut self,
        allocation: crate::AllocationSelection,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        match allocation {
            crate::AllocationSelection::CurrentRegion => Ok(true),
            crate::AllocationSelection::Explicit(value) => self.visit(value, state),
        }
    }

    fn visit_aggregate(
        &mut self,
        aggregate: &AggregateConstruction,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let values = match aggregate {
            AggregateConstruction::Struct { fields, .. } => {
                return self.visit_value_sequence(fields.iter().map(|(_, value)| *value), state);
            }
            AggregateConstruction::Enum { payload, .. }
            | AggregateConstruction::FixedArray(payload) => payload,
        };
        self.visit_value_sequence(values.iter().copied(), state)
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
                let actions = self.transfer_cleanup(state)?;
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
            CheckedControl::Pattern {
                subject,
                arms,
                fallback,
                unmatched,
            } => self.visit_pattern(node, *subject, arms, *fallback, *unmatched, state),
            CheckedControl::Region {
                binding,
                allocator,
                body,
            } => self.visit_region(*binding, *allocator, *body, state),
        }
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
            let retained_temporaries = state.temporary_identities();
            if !self.visit(*statement, state)? {
                reaches = false;
                break;
            }
            let actions = self.temporary_cleanup_actions(state, &retained_temporaries)?;
            self.record_cleanup(*statement, CleanupTiming::AtStatementEnd, actions);
            state.forget_temporaries_except(&retained_temporaries);
        }
        if reaches && let Some(result) = result {
            reaches = self.visit(result, state)?;
        }
        if reaches {
            let mut actions = self.scope_lifetime_cleanup(scope, state)?;
            if self.scopes.len() == 1 {
                let mut temporary_actions = self.temporary_cleanup_actions(state, &[])?;
                temporary_actions.append(&mut actions);
                actions = temporary_actions;
                actions.extend(self.execution_storage_cleanup(state)?);
            }
            self.record_cleanup(node, CleanupTiming::BeforeTransfer, actions);
        }
        self.leave_scope(scope)?;
        Ok(reaches)
    }

    fn visit_discard(
        &mut self,
        _node: BodyNodeId,
        value: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(value, state)? {
            return Ok(false);
        }
        self.activate_owned_temporary(value, state)?;
        Ok(true)
    }

    fn visit_region(
        &mut self,
        binding: LocalBindingId,
        allocator: BodyNodeId,
        body: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        if !self.visit(allocator, state)? {
            return Ok(false);
        }
        state
            .declare_initialized(MovePath::root(crate::PlaceRoot::Local(binding)))
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        let body_scope = self.block_scope(body)?;
        self.regions.push(RegionFlow {
            binding,
            body_scope,
            parent: allocator,
        });
        let reaches = self.visit(body, state)?;
        if self.regions.pop().is_none_or(|frame| {
            frame.binding != binding || frame.body_scope != body_scope || frame.parent != allocator
        }) {
            return Err(BodyCheckInternalError::CleanupPlanning.into());
        }
        Ok(reaches)
    }

    fn visit_if(
        &mut self,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let retained_temporaries = state.temporary_identities();
        if !self.visit(condition, state)? {
            return Ok(false);
        }
        let actions = self.temporary_cleanup_actions(state, &retained_temporaries)?;
        self.record_cleanup(condition, CleanupTiming::AtControlHeaderEnd, actions);
        state.forget_temporaries_except(&retained_temporaries);
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
        let body_scope = frame.body_scope;
        let iterator = frame.iterator;
        let retained = frame.retained_temporaries.clone();
        let mut actions = self.temporary_cleanup_actions(state, &retained)?;
        state.forget_temporaries_except(&retained);
        actions.extend(self.loop_scope_cleanup(body_scope, state)?);
        if is_break
            && let Some(iterator) = iterator
            && let Some(action) = self.consume_temporary_cleanup(iterator, state)?
        {
            actions.push(action);
        }
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
                            | OwnershipStateError::NotInitialized { .. }
                            | OwnershipStateError::DuplicateTemporary(_)
                            | OwnershipStateError::UnavailableTemporary(_) => {
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
        let iterator = if let LoopKind::For { iteration, .. } = definition.kind() {
            if !self.visit(iteration.iterator(), state)? {
                return Ok(false);
            }
            self.activate_expression_temporary(iteration.iterator(), state)?
                .then_some(TemporaryIdentity::Value(iteration.iterator()))
        } else {
            None
        };
        let body_scope = self.block_scope(definition.body())?;
        let preheader = state.clone();
        let retained_temporaries = preheader.temporary_identities();
        let mut header = preheader.clone();
        loop {
            self.loops.push(LoopFlow {
                id: loop_,
                body_scope,
                iterator,
                retained_temporaries: retained_temporaries.clone(),
                breaks: Vec::new(),
                continues: Vec::new(),
            });
            let mut iteration = header.clone();
            let retained_condition_temporaries = iteration.temporary_identities();
            let condition_reaches = match definition.kind() {
                LoopKind::While { condition } => self.visit(*condition, &mut iteration)?,
                LoopKind::Infinite
                | LoopKind::Range { .. }
                | LoopKind::For { .. }
                | LoopKind::LiteralPack { .. } => true,
            };
            if condition_reaches && let LoopKind::While { condition } = definition.kind() {
                let actions =
                    self.temporary_cleanup_actions(&iteration, &retained_condition_temporaries)?;
                self.record_cleanup(*condition, CleanupTiming::AtControlHeaderEnd, actions);
                iteration.forget_temporaries_except(&retained_condition_temporaries);
            }
            let condition_exit = (condition_reaches
                && matches!(
                    definition.kind(),
                    LoopKind::While { .. }
                        | LoopKind::Range { .. }
                        | LoopKind::For { .. }
                        | LoopKind::LiteralPack { .. }
                ))
            .then(|| iteration.clone());
            if condition_reaches
                && let LoopKind::Range { binding, .. }
                | LoopKind::For { binding, .. }
                | LoopKind::LiteralPack { binding, .. } = definition.kind()
            {
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

    /// Cleans ordinary owned values in a lexical scope, then releases the child allocation
    /// context whose lifetime is defined by that scope, if any.
    fn scope_lifetime_cleanup(
        &mut self,
        scope: BodyScopeId,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut actions = self.scope_cleanup(scope, state)?;
        let Some(region) = self
            .regions
            .iter()
            .rev()
            .find(|region| region.body_scope == scope)
            .copied()
        else {
            return Ok(actions);
        };
        let root = crate::PlaceRoot::Local(region.binding);
        if !state.contains_root(root) {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        state.forget_root(root);
        actions.push(CleanupAction::new(
            CleanupTarget::Region {
                binding: region.binding,
                parent: region.parent,
            },
            CleanupCondition::Always,
        ));
        Ok(actions)
    }

    fn execution_storage_cleanup(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut planner = CleanupPlanner::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.body,
            self.source,
        );
        match self.closure {
            Some(closure) if closure.signature().capability() == CallableCapability::Owned => {
                planner.closure_capture_actions(closure, state)
            }
            Some(_) => Ok(Vec::new()),
            None => planner.parameter_actions(state),
        }
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

    pub(super) fn transfer_cleanup(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let retained = self.active_loop_temporaries();
        let mut actions = self.temporary_cleanup_actions(state, &retained)?;
        state.forget_temporaries_except(&retained);
        let loop_lifetimes = self
            .loops
            .iter()
            .filter_map(|frame| frame.iterator.map(|iterator| (frame.body_scope, iterator)))
            .collect::<Vec<_>>();
        let scopes = self.scopes.clone();
        for scope in scopes.into_iter().rev() {
            actions.extend(self.scope_lifetime_cleanup(scope, state)?);
            if let Some((_, iterator)) = loop_lifetimes
                .iter()
                .rev()
                .find(|(body_scope, _)| *body_scope == scope)
                && let Some(action) = self.consume_temporary_cleanup(*iterator, state)?
            {
                actions.push(action);
            }
        }
        actions.extend(self.execution_storage_cleanup(state)?);
        Ok(actions)
    }

    fn active_loop_temporaries(&self) -> Vec<TemporaryIdentity> {
        let mut retained = self
            .loops
            .iter()
            .filter_map(|frame| frame.iterator)
            .collect::<Vec<_>>();
        retained.sort_unstable();
        retained
    }

    fn consume_temporary_cleanup(
        &self,
        identity: TemporaryIdentity,
        state: &mut OwnershipState,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        let action = self.temporaries.cleanup_action(identity, state)?;
        if action.is_some() {
            state
                .consume_temporary(identity)
                .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        }
        Ok(action)
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
            actions.extend(self.scope_lifetime_cleanup(scope, state)?);
        }
        Ok(actions)
    }

    fn record_cleanup(
        &mut self,
        node: BodyNodeId,
        timing: CleanupTiming,
        actions: Vec<CleanupAction>,
    ) {
        let schedules = self.cleanup_schedules.entry(node).or_default();
        if let Some(index) = schedules
            .iter()
            .position(|schedule| schedule.timing() == timing)
        {
            if actions.is_empty() {
                schedules.remove(index);
            } else {
                schedules[index] = CleanupSchedule::new(timing, actions);
            }
        } else if !actions.is_empty() {
            schedules.push(CleanupSchedule::new(timing, actions));
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
                | OwnershipStateError::UnavailableAssignmentParent { .. }
                | OwnershipStateError::DuplicateTemporary(_)
                | OwnershipStateError::UnavailableTemporary(_),
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
