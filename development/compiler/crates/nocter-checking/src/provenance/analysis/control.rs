use nocter_model::BodyNodeId;

use super::{Analyzer, LoopFlow};
use crate::provenance::state::ProvenanceState;
use crate::{
    BodyCheckError, BodyCheckInternalError, CheckedControl, CheckedPatternArm,
    CheckedPatternFallback, CheckedPatternSubject, LoopKind, PlaceRoot, ProvenanceProjection,
    ProvenanceSource, ValueProvenance,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        match control {
            CheckedControl::Block {
                scope,
                statements,
                result,
            } => self.evaluate_block(node, *scope, statements, *result, state),
            CheckedControl::Bind {
                binding,
                initializer,
            } => self.evaluate_binding(node, *binding, *initializer, state),
            CheckedControl::Assign { target, value } => {
                self.evaluate_assignment(node, *target, *value, state)
            }
            CheckedControl::CompoundAssign { target, value, .. } => {
                let (_, reaches) = self.evaluate(*value, state)?;
                if reaches {
                    self.evaluate_place_indices(*target, state)?;
                }
                Ok((ValueProvenance::independent(), reaches))
            }
            CheckedControl::Discard(value) => {
                let (_, reaches) = self.evaluate(*value, state)?;
                Ok((ValueProvenance::independent(), reaches))
            }
            CheckedControl::Unreachable(_) => Ok((ValueProvenance::independent(), false)),
            CheckedControl::Return(value) => self.evaluate_return(node, *value, state),
            CheckedControl::Break(loop_) => self.evaluate_loop_control(*loop_, true, state),
            CheckedControl::Continue(loop_) => self.evaluate_loop_control(*loop_, false, state),
            CheckedControl::Drop(place) => {
                self.evaluate_place_indices(*place, state)?;
                self.remove_place(*place, state)?;
                Ok((ValueProvenance::independent(), true))
            }
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => self.evaluate_if(*condition, *then_branch, *else_branch, state),
            CheckedControl::Logical { left, right, .. } => {
                self.evaluate_logical(*left, *right, state)
            }
            CheckedControl::Pattern {
                subject,
                arms,
                fallback,
                unmatched,
            } => self.evaluate_pattern(*subject, arms, *fallback, *unmatched, state),
            CheckedControl::Loop(loop_) => self.evaluate_loop(*loop_, state),
            CheckedControl::Region {
                binding,
                allocator,
                body,
            } => self.evaluate_region(node, *binding, *allocator, *body, state),
        }
    }

    fn evaluate_block(
        &mut self,
        node: BodyNodeId,
        scope: nocter_model::BodyScopeId,
        statements: &[BodyNodeId],
        result: Option<BodyNodeId>,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        for statement in statements {
            if !self.evaluate(*statement, state)?.1 {
                self.remove_scope_locals(scope, state);
                return Ok((ValueProvenance::independent(), false));
            }
            self.validate_statement_storage(*statement, state)?;
        }
        let value = if let Some(result) = result {
            let (value, reaches) = self.evaluate(result, state)?;
            if !reaches {
                self.remove_scope_locals(scope, state);
                return Ok((ValueProvenance::independent(), false));
            }
            value
        } else {
            ValueProvenance::independent()
        };
        self.validate_scope_result(node, scope, &value)?;
        self.remove_scope_locals(scope, state);
        Ok((value, true))
    }

    fn evaluate_binding(
        &mut self,
        node: BodyNodeId,
        binding: nocter_model::LocalBindingId,
        initializer: BodyNodeId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let (value, reaches) = self.evaluate(initializer, state)?;
        if reaches {
            self.validate_binding_storage(node, binding, &value)?;
            state.set_value(PlaceRoot::Local(binding), value);
        }
        Ok((ValueProvenance::independent(), reaches))
    }

    fn evaluate_assignment(
        &mut self,
        node: BodyNodeId,
        target: nocter_model::PlaceId,
        value: BodyNodeId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let (value, reaches) = self.evaluate(value, state)?;
        if !reaches {
            return Ok((ValueProvenance::independent(), false));
        }
        self.evaluate_place_indices(target, state)?;
        self.validate_assignment_storage(node, target, &value)?;
        self.write_place(target, value, state)?;
        Ok((ValueProvenance::independent(), true))
    }

    fn evaluate_return(
        &mut self,
        node: BodyNodeId,
        value: Option<BodyNodeId>,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let returned = if let Some(value) = value {
            let (value, reaches) = self.evaluate(value, state)?;
            if !reaches {
                return Ok((ValueProvenance::independent(), false));
            }
            value
        } else {
            ValueProvenance::independent()
        };
        self.record_return(node, returned);
        Ok((ValueProvenance::independent(), false))
    }

    fn evaluate_loop_control(
        &mut self,
        loop_: nocter_model::LoopId,
        is_break: bool,
        state: &ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let frame = self
            .loops
            .last_mut()
            .filter(|frame| frame.id == loop_)
            .ok_or(BodyCheckInternalError::LoopStack)?;
        if is_break {
            frame.breaks.push(state.clone());
        } else {
            frame.continues.push(state.clone());
        }
        Ok((ValueProvenance::independent(), false))
    }

    fn evaluate_logical(
        &mut self,
        left: BodyNodeId,
        right: BodyNodeId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        if !self.evaluate(left, state)?.1 {
            return Ok((ValueProvenance::independent(), false));
        }
        let entry = state.clone();
        let mut right_state = entry.clone();
        let (_, right_reaches) = self.evaluate(right, &mut right_state)?;
        let mut incoming = vec![entry];
        if right_reaches {
            incoming.push(right_state);
        }
        state.join(&incoming);
        Ok((ValueProvenance::independent(), true))
    }

    fn evaluate_region(
        &mut self,
        node: BodyNodeId,
        binding: nocter_model::LocalBindingId,
        allocator: BodyNodeId,
        body: BodyNodeId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let (_, reaches) = self.evaluate(allocator, state)?;
        if !reaches {
            return Ok((ValueProvenance::independent(), false));
        }
        state.set_value(
            PlaceRoot::Local(binding),
            ValueProvenance::from_source(ProvenanceSource::Region(binding)),
        );
        let previous = state.enter_region(binding);
        let result = self.evaluate(body, state)?;
        state.leave_region(previous);
        state.remove(PlaceRoot::Local(binding));
        self.validate_region_exit(node, binding, &result.0, state)?;
        Ok(result)
    }

    fn evaluate_if(
        &mut self,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        if !self.evaluate(condition, state)?.1 {
            return Ok((ValueProvenance::independent(), false));
        }
        let entry = state.clone();
        let mut incoming = Vec::new();
        let mut result = ValueProvenance::independent();
        let mut then_state = entry.clone();
        let (then_value, then_reaches) = self.evaluate(then_branch, &mut then_state)?;
        if then_reaches {
            result.union_with(&then_value);
            incoming.push(then_state);
        }
        let mut else_state = entry.clone();
        if let Some(else_branch) = else_branch {
            let (else_value, else_reaches) = self.evaluate(else_branch, &mut else_state)?;
            if else_reaches {
                result.union_with(&else_value);
                incoming.push(else_state);
            }
        } else {
            incoming.push(else_state);
        }
        *state = entry;
        state.join(&incoming);
        Ok((result, !incoming.is_empty()))
    }

    fn evaluate_pattern(
        &mut self,
        subject: CheckedPatternSubject,
        arms: &[CheckedPatternArm],
        fallback: Option<CheckedPatternFallback>,
        unmatched: bool,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let (subject_value, reaches) = self.evaluate(subject.value(), state)?;
        if !reaches {
            return Ok((ValueProvenance::independent(), false));
        }
        let entry = state.clone();
        let mut incoming = Vec::new();
        let mut result = ValueProvenance::independent();
        for arm in arms {
            let mut arm_state = entry.clone();
            Self::bind_pattern_arm(&subject_value, arm, &mut arm_state);
            let (value, reaches) = self.evaluate(arm.body(), &mut arm_state)?;
            if reaches {
                result.union_with(&value);
                incoming.push(arm_state);
            }
        }
        if let Some(fallback) = fallback {
            if fallback.reachable() {
                let mut fallback_state = entry.clone();
                let (value, reaches) = self.evaluate(fallback.body(), &mut fallback_state)?;
                if reaches {
                    result.union_with(&value);
                    incoming.push(fallback_state);
                }
            }
        } else if unmatched {
            incoming.push(entry.clone());
        }
        *state = entry;
        state.join(&incoming);
        Ok((result, !incoming.is_empty()))
    }

    fn bind_pattern_arm(
        subject_value: &ValueProvenance,
        arm: &CheckedPatternArm,
        state: &mut ProvenanceState,
    ) {
        for slot in arm.pattern().slots() {
            let Some(binding) = slot.binding() else {
                continue;
            };
            let payload = subject_value.projected(ProvenanceProjection::VariantPayload {
                variant: arm.pattern().variant(),
                parameter: slot.parameter(),
            });
            state.set_value(PlaceRoot::Local(binding), payload);
        }
    }

    fn evaluate_loop(
        &mut self,
        loop_: nocter_model::LoopId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let definition = self
            .body
            .loops()
            .get(loop_)
            .ok_or(BodyCheckInternalError::UnknownLoop(loop_))?
            .clone();
        if let LoopKind::Range { start, end, .. } = definition.kind()
            && (!self.evaluate(*start, state)?.1 || !self.evaluate(*end, state)?.1)
        {
            return Ok((ValueProvenance::independent(), false));
        }
        let iterator = if let LoopKind::For { iteration, .. } = definition.kind() {
            let (value, reaches) = self.evaluate(iteration.iterator(), state)?;
            if !reaches {
                return Ok((ValueProvenance::independent(), false));
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
                breaks: Vec::new(),
                continues: Vec::new(),
            });
            let mut iteration = header.clone();
            let condition_reaches = match definition.kind() {
                LoopKind::While { condition } => self.evaluate(*condition, &mut iteration)?.1,
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
                self.bind_loop_iteration(
                    definition.kind(),
                    iterator.as_ref(),
                    definition.body_scope(),
                    &mut iteration,
                )?;
            }
            let body_reaches =
                condition_reaches && self.evaluate(definition.body(), &mut iteration)?.1;
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
            *state = preheader.clone();
            state.join(&exits);
            return Ok((ValueProvenance::independent(), !exits.is_empty()));
        }
    }

    fn bind_loop_iteration(
        &self,
        kind: &LoopKind,
        iterator: Option<&ValueProvenance>,
        body_scope: nocter_model::BodyScopeId,
        state: &mut ProvenanceState,
    ) -> Result<(), BodyCheckInternalError> {
        match kind {
            LoopKind::Range { binding, .. } => {
                state.set_value(PlaceRoot::Local(*binding), ValueProvenance::independent());
            }
            LoopKind::For { binding, iteration } => {
                let value = self.iteration_item_provenance(
                    iteration,
                    iterator.ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    state.current_allocation(),
                    ProvenanceSource::ScopedTemporary {
                        value: iteration.iterator(),
                        scope: body_scope,
                    },
                )?;
                state.set_value(PlaceRoot::Local(*binding), value);
            }
            LoopKind::ArgumentPack {
                binding, parameter, ..
            } => {
                let value = state.value(PlaceRoot::Parameter(*parameter));
                state.set_value(PlaceRoot::Local(*binding), value);
            }
            LoopKind::KeyedArgumentPack {
                key_binding,
                value_binding,
                parameter,
                ..
            } => {
                let value = state.value(PlaceRoot::Parameter(*parameter));
                state.set_value(PlaceRoot::Local(*key_binding), value.clone());
                state.set_value(PlaceRoot::Local(*value_binding), value);
            }
            LoopKind::Infinite | LoopKind::While { .. } => {}
        }
        Ok(())
    }
}
