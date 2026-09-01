use nocter_model::{BodyNodeId, BuiltinType, LocalBindingId, TypeId};

use super::OwnershipAnalyzer;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::ownership::{MovePath, OwnershipState, TemporaryIdentity};
use crate::{
    CheckedPattern, CheckedPatternArm, CheckedPatternFallback, CheckedPatternSubject,
    CleanupAction, CleanupTiming, PatternSubjectPreparation, PlaceRoot,
};

impl OwnershipAnalyzer<'_> {
    pub(super) fn visit_pattern(
        &mut self,
        node: BodyNodeId,
        subject: CheckedPatternSubject,
        arms: &[CheckedPatternArm],
        fallback: Option<CheckedPatternFallback>,
        unmatched: bool,
        state: &mut OwnershipState,
    ) -> Result<bool, BodyCheckError> {
        let retained_temporaries = state.temporary_identities();
        if !self.visit(subject.value(), state)? {
            return Ok(false);
        }
        let actions = self.temporary_cleanup_actions(state, &retained_temporaries)?;
        self.record_cleanup(subject.value(), CleanupTiming::AtControlHeaderEnd, actions);
        state.forget_temporaries_except(&retained_temporaries);

        let entry = state.clone();
        let mut incoming = Vec::new();
        for arm in arms {
            let mut branch = entry.clone();
            self.prepare_pattern_branch(
                TemporaryIdentity::PatternResidual(arm.body()),
                subject,
                Some(arm.pattern()),
                &mut branch,
            )?;
            self.initialize_pattern_bindings(arm.pattern(), &mut branch)?;
            if self.visit(arm.body(), &mut branch)? {
                incoming.push(branch);
            }
        }
        if let Some(fallback) = fallback {
            let mut branch = entry.clone();
            self.prepare_pattern_branch(
                TemporaryIdentity::PatternResidual(fallback.body()),
                subject,
                None,
                &mut branch,
            )?;
            if fallback.reachable() {
                if self.visit(fallback.body(), &mut branch)? {
                    incoming.push(branch);
                }
            } else {
                self.visit_nonruntime_pattern_body(fallback.body(), &mut branch)?;
            }
        }
        if unmatched {
            let mut branch = entry.clone();
            self.prepare_pattern_branch(
                TemporaryIdentity::PatternUnmatched(node),
                subject,
                None,
                &mut branch,
            )?;
            incoming.push(branch);
        }
        *state = entry
            .join_reachable(&incoming)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?;
        Ok(!incoming.is_empty())
    }

    fn prepare_pattern_branch(
        &mut self,
        identity: TemporaryIdentity,
        subject: CheckedPatternSubject,
        pattern: Option<&CheckedPattern>,
        state: &mut OwnershipState,
    ) -> Result<(), BodyCheckInternalError> {
        if !matches!(
            subject.preparation(),
            PatternSubjectPreparation::OwnedTemporary | PatternSubjectPreparation::ConsumedPlace
        ) {
            return Ok(());
        }
        let ty = self.pattern_subject_type(subject)?;
        let action = if let Some(pattern) = pattern {
            self.enum_residual_cleanup(subject.value(), ty, pattern)?
        } else {
            self.value_cleanup(subject.value(), ty)?
        };
        self.temporaries.activate(identity, action, state)?;
        Ok(())
    }

    fn initialize_pattern_bindings(
        &self,
        pattern: &CheckedPattern,
        state: &mut OwnershipState,
    ) -> Result<(), BodyCheckInternalError> {
        for binding in pattern.slots().iter().filter_map(|slot| slot.binding()) {
            self.initialize_pattern_binding(binding, state)?;
        }
        Ok(())
    }

    fn initialize_pattern_binding(
        &self,
        binding: LocalBindingId,
        state: &mut OwnershipState,
    ) -> Result<(), BodyCheckInternalError> {
        if self.body.locals().get(binding).is_none() {
            return Err(BodyCheckInternalError::MissingLocalType(binding));
        }
        state
            .declare_initialized(MovePath::root(PlaceRoot::Local(binding)))
            .map_err(|_| BodyCheckInternalError::OwnershipState)
    }

    fn pattern_subject_type(
        &self,
        subject: CheckedPatternSubject,
    ) -> Result<TypeId, BodyCheckInternalError> {
        let ty = self
            .body
            .nodes()
            .get(subject.value())
            .map(crate::CheckedNode::ty)
            .ok_or(BodyCheckInternalError::MissingNode(subject.value()))?;
        if ty == self.types.builtin(BuiltinType::Never) {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        Ok(ty)
    }

    fn enum_residual_cleanup(
        &mut self,
        subject: BodyNodeId,
        ty: TypeId,
        pattern: &CheckedPattern,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        self.cleanup_planner()
            .enum_residual_action(subject, ty, pattern)
    }

    fn visit_nonruntime_pattern_body(
        &mut self,
        body: BodyNodeId,
        state: &mut OwnershipState,
    ) -> Result<(), BodyCheckError> {
        let loop_lengths = self
            .loops
            .iter()
            .map(|frame| (frame.breaks.len(), frame.continues.len()))
            .collect::<Vec<_>>();
        let _ = self.visit(body, state)?;
        if self.loops.len() != loop_lengths.len() {
            return Err(BodyCheckInternalError::LoopStack.into());
        }
        for (frame, (breaks, continues)) in self.loops.iter_mut().zip(loop_lengths) {
            frame.breaks.truncate(breaks);
            frame.continues.truncate(continues);
        }
        Ok(())
    }
}
