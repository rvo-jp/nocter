use std::collections::BTreeSet;

use nocter_model::{BodyNodeId, LoopId};

use super::{AccessKind, Analyzer, LoopFlow};
use crate::loans::liveness::{LivePlace, LiveSlot};
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    BodyCheckError, BodyCheckInternalError, CheckedControl, LoanId, LoopKind, PlaceRoot,
    ProvenanceProjection,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        match control {
            CheckedControl::Block {
                scope,
                statements,
                result,
            } => self.evaluate_block(node, *scope, statements, *result, state, extra),
            CheckedControl::Bind {
                binding,
                initializer,
            } => {
                let (value, reaches) = self.evaluate(*initializer, state, extra)?;
                if reaches {
                    state.set_root(PlaceRoot::Local(*binding), value);
                }
                Ok((LoanValue::independent(), reaches))
            }
            CheckedControl::Assign { target, value } => {
                self.evaluate_assignment(node, *target, *value, state, extra)
            }
            CheckedControl::CompoundAssign { target, value, .. } => {
                if !self.evaluate(*value, state, extra)?.1 {
                    return Ok((LoanValue::independent(), false));
                }
                self.evaluate_place_indices(*target, state, extra)?;
                self.check_place_access(node, *target, AccessKind::Write, state, extra)?;
                Ok((LoanValue::independent(), true))
            }
            CheckedControl::Discard(value) => {
                let (_, reaches) = self.evaluate(*value, state, extra)?;
                Ok((LoanValue::independent(), reaches))
            }
            CheckedControl::Unreachable(_) => Ok((LoanValue::independent(), false)),
            CheckedControl::Return(value) => {
                self.evaluate_return_transfer(node, *value, state, extra)
            }
            CheckedControl::Break(loop_) => {
                self.evaluate_loop_transfer(node, *loop_, true, state, extra)
            }
            CheckedControl::Continue(loop_) => {
                self.evaluate_loop_transfer(node, *loop_, false, state, extra)
            }
            CheckedControl::Drop(place) => {
                self.evaluate_place_indices(*place, state, extra)?;
                self.check_place_access(node, *place, AccessKind::Write, state, extra)?;
                self.remove_place(*place, state)?;
                Ok((LoanValue::independent(), true))
            }
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => self.evaluate_if(*condition, *then_branch, *else_branch, state, extra),
            CheckedControl::Logical { left, right, .. } => {
                if !self.evaluate(*left, state, extra)?.1 {
                    return Ok((LoanValue::independent(), false));
                }
                let entry = state.clone();
                let mut right_state = entry.clone();
                self.evaluate(*right, &mut right_state, extra)?;
                state.join(&[entry, right_state]);
                Ok((LoanValue::independent(), true))
            }
            CheckedControl::Pattern {
                subject,
                arms,
                fallback,
                unmatched,
            } => self.evaluate_pattern(*subject, arms, *fallback, *unmatched, state, extra),
            CheckedControl::Loop(loop_) => self.evaluate_loop(*loop_, state, extra),
            CheckedControl::Region {
                binding,
                allocator,
                body,
            } => {
                if !self.evaluate(*allocator, state, extra)?.1 {
                    return Ok((LoanValue::independent(), false));
                }
                state.set_root(PlaceRoot::Local(*binding), LoanValue::independent());
                let output = self.evaluate(*body, state, extra)?;
                state.remove_root(PlaceRoot::Local(*binding));
                Ok(output)
            }
        }
    }

    fn evaluate_block(
        &mut self,
        node: BodyNodeId,
        scope: nocter_model::BodyScopeId,
        statements: &[BodyNodeId],
        result: Option<BodyNodeId>,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        self.scopes.push(scope);
        for statement in statements {
            if !self.evaluate(*statement, state, extra)?.1 {
                self.remove_scope_roots(scope, state);
                if self.scopes.pop() != Some(scope) {
                    return Err(BodyCheckInternalError::LoanAnalysis.into());
                }
                return Ok((LoanValue::independent(), false));
            }
        }
        let output = if let Some(result) = result {
            self.evaluate(result, state, extra)?
        } else {
            (LoanValue::independent(), true)
        };
        self.check_scope_exit_conflicts(node, [scope], state, extra)?;
        self.check_cleanup_conflicts(node, state, extra)?;
        self.remove_scope_roots(scope, state);
        if self.scopes.pop() != Some(scope) {
            return Err(BodyCheckInternalError::LoanAnalysis.into());
        }
        Ok(output)
    }

    fn evaluate_assignment(
        &mut self,
        node: BodyNodeId,
        target: nocter_model::PlaceId,
        value: BodyNodeId,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let (value, reaches) = self.evaluate(value, state, extra)?;
        if !reaches {
            return Ok((LoanValue::independent(), false));
        }
        self.evaluate_place_indices(target, state, extra)?;
        self.check_place_access(node, target, AccessKind::Write, state, extra)?;
        self.check_cleanup_conflicts(node, state, extra)?;
        let place = self
            .input
            .body
            .places()
            .get(target)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(target))?;
        state.set_place(&LivePlace::from_checked(place), value);
        Ok((LoanValue::independent(), true))
    }

    fn evaluate_return_transfer(
        &mut self,
        node: BodyNodeId,
        value: Option<BodyNodeId>,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        if let Some(value) = value {
            self.evaluate(value, state, extra)?;
        }
        self.check_scope_exit_conflicts(node, self.scopes.iter().rev().copied(), state, extra)?;
        self.check_cleanup_conflicts(node, state, extra)?;
        Ok((LoanValue::independent(), false))
    }

    fn evaluate_loop_transfer(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        is_break: bool,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let depth = self.loop_scope_depth(loop_)?;
        self.check_scope_exit_conflicts(
            node,
            self.scopes[depth..].iter().rev().copied(),
            state,
            extra,
        )?;
        self.check_cleanup_conflicts(node, state, extra)?;
        let frame = self.loop_frame_mut(loop_)?;
        if is_break {
            frame.breaks.push(state.clone());
        } else {
            frame.continues.push(state.clone());
        }
        Ok((LoanValue::independent(), false))
    }

    fn evaluate_if(
        &mut self,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        if !self.evaluate(condition, state, extra)?.1 {
            return Ok((LoanValue::independent(), false));
        }
        let entry = state.clone();
        let mut incoming = Vec::new();
        let mut output = LoanValue::independent();
        let mut then_state = entry.clone();
        let (then_value, then_reaches) = self.evaluate(then_branch, &mut then_state, extra)?;
        if then_reaches {
            incoming.push(then_state);
            output.union_with(&then_value);
        }
        let mut else_state = entry.clone();
        if let Some(else_branch) = else_branch {
            let (else_value, else_reaches) = self.evaluate(else_branch, &mut else_state, extra)?;
            if else_reaches {
                incoming.push(else_state);
                output.union_with(&else_value);
            }
        } else {
            incoming.push(else_state);
        }
        *state = entry;
        state.join(&incoming);
        Ok((output, !incoming.is_empty()))
    }

    pub(super) fn evaluate_pattern(
        &mut self,
        subject: crate::CheckedPatternSubject,
        arms: &[crate::CheckedPatternArm],
        fallback: Option<crate::CheckedPatternFallback>,
        unmatched: bool,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let (subject_value, reaches) = self.evaluate(subject.value(), state, extra)?;
        if !reaches {
            return Ok((LoanValue::independent(), false));
        }
        let entry = state.clone();
        let mut incoming = Vec::new();
        let mut output = LoanValue::independent();
        for arm in arms {
            let mut arm_state = entry.clone();
            for slot in arm.pattern().slots() {
                if let Some(binding) = slot.binding() {
                    let value = subject_value.projected(ProvenanceProjection::VariantPayload {
                        variant: arm.pattern().variant(),
                        parameter: slot.parameter(),
                    });
                    arm_state.set_root(PlaceRoot::Local(binding), value);
                }
            }
            let (value, arm_reaches) = self.evaluate(arm.body(), &mut arm_state, extra)?;
            for slot in arm.pattern().slots() {
                if let Some(binding) = slot.binding() {
                    arm_state.remove_root(PlaceRoot::Local(binding));
                }
            }
            if arm_reaches {
                incoming.push(arm_state);
                output.union_with(&value);
            }
        }
        if let Some(fallback) = fallback
            && fallback.reachable()
        {
            let mut fallback_state = entry.clone();
            let (value, reaches) = self.evaluate(fallback.body(), &mut fallback_state, extra)?;
            if reaches {
                incoming.push(fallback_state);
                output.union_with(&value);
            }
        } else if unmatched {
            incoming.push(entry.clone());
        }
        *state = entry;
        state.join(&incoming);
        Ok((output, !incoming.is_empty()))
    }

    pub(super) fn evaluate_loop(
        &mut self,
        loop_: LoopId,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let definition = self
            .input
            .body
            .loops()
            .get(loop_)
            .cloned()
            .ok_or(BodyCheckInternalError::UnknownLoop(loop_))?;
        if let LoopKind::Range { start, end, .. } = definition.kind()
            && (!self.evaluate(*start, state, extra)?.1 || !self.evaluate(*end, state, extra)?.1)
        {
            return Ok((LoanValue::independent(), false));
        }
        let iterator = if let LoopKind::For { iteration, .. } = definition.kind() {
            let (value, reaches) = self.evaluate(iteration.iterator(), state, extra)?;
            if !reaches {
                return Ok((LoanValue::independent(), false));
            }
            Some(value)
        } else {
            None
        };
        let preheader = state.clone();
        let mut header = preheader.clone();
        loop {
            self.loops.push(LoopFlow {
                id: loop_,
                scope_depth: self.scopes.len(),
                breaks: Vec::new(),
                continues: Vec::new(),
            });
            let mut iteration = header.clone();
            let condition_reaches = match definition.kind() {
                LoopKind::While { condition } => {
                    self.evaluate(*condition, &mut iteration, extra)?.1
                }
                LoopKind::Infinite
                | LoopKind::Range { .. }
                | LoopKind::For { .. }
                | LoopKind::ArgumentPack { .. }
                | LoopKind::KeyedArgumentPack { .. } => true,
            };
            let condition_exit = (condition_reaches
                && matches!(
                    definition.kind(),
                    LoopKind::While { .. }
                        | LoopKind::Range { .. }
                        | LoopKind::For { .. }
                        | LoopKind::ArgumentPack { .. }
                        | LoopKind::KeyedArgumentPack { .. }
                ))
            .then(|| iteration.clone());
            if condition_reaches {
                self.initialize_loop_bindings(
                    definition.kind(),
                    iterator.as_ref(),
                    &mut iteration,
                )?;
            }
            let body_reaches =
                condition_reaches && self.evaluate(definition.body(), &mut iteration, extra)?.1;
            let mut frame = self.loops.pop().ok_or(BodyCheckInternalError::LoopStack)?;
            if body_reaches {
                frame.continues.push(iteration);
            }
            let mut header_incoming = vec![preheader.clone()];
            header_incoming.extend(frame.continues);
            let mut next_header = preheader.clone();
            next_header.join(&header_incoming);
            if next_header != header {
                header = next_header;
                continue;
            }
            let mut exits = frame.breaks;
            if let Some(condition_exit) = condition_exit {
                exits.push(condition_exit);
            }
            *state = preheader;
            state.join(&exits);
            return Ok((LoanValue::independent(), !exits.is_empty()));
        }
    }

    fn initialize_loop_bindings(
        &self,
        kind: &LoopKind,
        iterator: Option<&LoanValue>,
        state: &mut LoanState,
    ) -> Result<(), BodyCheckError> {
        match kind {
            LoopKind::While { .. } | LoopKind::Infinite => {}
            LoopKind::Range { binding, .. } => {
                state.set_root(PlaceRoot::Local(*binding), LoanValue::independent());
            }
            LoopKind::For { binding, iteration } => {
                let value = self.iteration_item_loans(
                    iteration,
                    iterator.ok_or(BodyCheckInternalError::LoanAnalysis)?,
                )?;
                state.set_root(PlaceRoot::Local(*binding), value);
            }
            LoopKind::ArgumentPack {
                binding, parameter, ..
            } => {
                let value = argument_pack_parameter_loans(state, *parameter);
                state.set_root(PlaceRoot::Local(*binding), value);
            }
            LoopKind::KeyedArgumentPack {
                key_binding,
                value_binding,
                parameter,
                ..
            } => {
                let value = argument_pack_parameter_loans(state, *parameter);
                state.set_root(PlaceRoot::Local(*key_binding), value.clone());
                state.set_root(PlaceRoot::Local(*value_binding), value);
            }
        }
        Ok(())
    }

    pub(super) fn loop_frame_mut(
        &mut self,
        loop_: LoopId,
    ) -> Result<&mut LoopFlow, BodyCheckInternalError> {
        self.loops
            .iter_mut()
            .rev()
            .find(|frame| frame.id == loop_)
            .ok_or(BodyCheckInternalError::LoopStack)
    }

    fn loop_scope_depth(&self, loop_: LoopId) -> Result<usize, BodyCheckInternalError> {
        self.loops
            .iter()
            .rev()
            .find(|frame| frame.id == loop_)
            .map(|frame| frame.scope_depth)
            .ok_or(BodyCheckInternalError::LoopStack)
    }

    pub(super) fn remove_scope_roots(
        &self,
        scope: nocter_model::BodyScopeId,
        state: &mut LoanState,
    ) {
        for (local, definition) in self.input.body.locals().iter() {
            if definition.declaration().scope() == scope {
                state.remove_root(PlaceRoot::Local(local));
            }
        }
    }
}

fn argument_pack_parameter_loans(
    state: &LoanState,
    parameter: nocter_model::ParameterId,
) -> LoanValue {
    state.value(&LiveSlot::Place(LivePlace::from_parts(
        PlaceRoot::Parameter(parameter),
        Box::new([]),
    )))
}
